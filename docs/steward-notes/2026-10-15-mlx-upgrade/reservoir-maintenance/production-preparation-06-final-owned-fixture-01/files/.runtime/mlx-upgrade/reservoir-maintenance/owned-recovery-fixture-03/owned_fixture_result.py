"""Bounded retained-file aggregate and final read-only owned observations.

No work occurs on import. The parent calls collect_final only while holding the
shared transaction lock, after all71 executor children have naturally exited.
The independent production reader revalidates these raw files; this producer's
summary does not authorize production execution.
"""
from __future__ import annotations

import os
from pathlib import Path
import stat
import time

import owned_fixture_records as r

MAX_FILES = 5000
MAX_TOTAL_BYTES = 64 * 1024 * 1024
LOCK_NAMES = ('setup.lock', 'execution.lock')
STATE = {'fixture_astrid': {'ticks': 22}, 'fixture_minime': {'ticks': 7}}
BASELINE = {'fixture_astrid': {'ticks': 11}, 'fixture_minime': {'ticks': 7}}


def inventory(root):
    """Exact finite regular-file set; only the two explicit lock paths excluded."""
    root = r.directory(root)
    result = {}; excluded = []; total = 0; directory_count = 0
    for parent, directories, files in os.walk(root, followlinks=False):
        directory_count += 1
        r.require(directory_count <= MAX_FILES, 'fixture directory count exceeds finite bound')
        for name in directories:
            r.directory(Path(parent) / name)
        for name in sorted(files):
            path = Path(parent) / name
            info = path.lstat()
            r.require(stat.S_ISREG(info.st_mode) and not path.is_symlink()
                      and info.st_nlink == 1 and info.st_uid == os.getuid(), 'unsafe retained file')
            if path.parent == root and name in LOCK_NAMES:
                r.require(info.st_size == 0, 'lock file contains unexpected payload')
                excluded.append(str(path)); continue
            total += info.st_size
            r.require(len(result) < MAX_FILES and total <= MAX_TOTAL_BYTES, 'fixture inventory exceeds finite bounds')
            result[str(path)] = r.reference(path)['sha256']
    return dict(sorted(result.items())), sorted(excluded)


def collect_identities(root, parent_identity):
    """Collect every recorded full identity; reject PID reuse instead of guessing."""
    identities = {}
    def visit(value):
        if isinstance(value, dict):
            if set(value) == {'pid', 'started_at', 'argv'}:
                r.identity(value)
                if value['pid'] == parent_identity['pid']:
                    r.require(value == parent_identity, 'parent PID reused in retained identities')
                else:
                    key = str(value['pid'])
                    r.require(key not in identities or identities[key] == value, 'owned PID reused during fixture')
                    identities[key] = value
            else:
                for child in value.values(): visit(child)
        elif isinstance(value, list):
            for child in value: visit(child)
    artifacts, _ = inventory(root)
    for filename in artifacts:
        if Path(filename).suffix == '.json':
            visit(r.read(filename))
    return identities


def executor_records(root, release):
    result = []
    wanted = [row['id'] for row in release['executor_plan']]
    actual = sorted(path.name for path in (root / 'executors').iterdir())
    r.require(actual == sorted(wanted), 'missing or extra executor directory')
    names = {'request': 'request.json', 'launch': 'launch.json', 'created': 'child-created.json',
             'self_start': 'self-start.json', 'exit': 'exit.json', 'result': 'result.json',
             'injection_ack': 'injection-ack.json', 'lock_held': 'lock-held.json',
             'lock_refusal': 'lock-refusal.json', 'release_lock': 'release-lock.json'}
    for step in release['executor_plan']:
        directory = r.directory(root / 'executors' / step['id'])
        r.require(not (directory / 'unresolved.json').exists(), 'unresolved executor cannot be accepted')
        row = {'step': step, **{key: r.reference(directory / filename) if (directory / filename).exists() else None
                               for key, filename in names.items()}}
        for key in ('request', 'launch', 'created', 'self_start', 'exit'):
            r.require(row[key] is not None, 'executor lacks required raw record: ' + key)
        request = r.check_reference(row['request']); exit_record = r.check_reference(row['exit'])
        started = r.check_reference(row['self_start'])
        r.require(request['step'] == step and request['root'] == str(root)
                  and started['request'] == row['request'] and exit_record['start'] == started
                  and exit_record['pid'] == started['identity']['pid']
                  and exit_record['returncode'] == step['expected_exit']
                  and exit_record['reaped'] is True and exit_record['timed_out'] is False
                  and exit_record['signals_sent'] == 0, 'executor natural exit differs')
        if step['expected_exit'] == 75:
            r.require(row['injection_ack'] is not None and row['result'] is None, 'injected executor raw outcome differs')
            from owned_fixture_injection import verify_executor_exit
            points = r.check_reference(release['injections'])['points']
            point = next(item for item in points if item['id'] == step['injection'])
            verify_executor_exit(started, exit_record, r.check_reference(row['injection_ack']), point, request['release'])
        elif step['kind'] == 'lock_refusal':
            r.require(row['lock_refusal'] is not None and r.check_reference(row['lock_refusal'])['action_performed'] is False,
                      'lock refusal raw evidence missing')
        elif step['kind'] == 'lock_hold':
            r.require(row['lock_held'] is not None and row['release_lock'] is not None, 'lock-holder raw evidence missing')
        else:
            r.require(row['result'] is not None and row['injection_ack'] is None, 'ordinary executor raw outcome missing')
        result.append(row)
    return result


def collect_final(host, core, release):
    core['transaction_execution'].require_active(host.root, host.plan)
    from owned_fixture_cleanup import ACTIONS, attach
    from owned_fixture_pin import ACTIONS as PIN_ACTIONS
    from owned_pin_system import PinSystem
    context, cleanup = attach(host, core)
    r.require(list(cleanup.completed()) == list(ACTIONS) and cleanup.completed()['complete']['passed'] is True,
              'separate cleanup is not complete')
    pin_inputs = r.read(host.root / 'pin/inputs.json')
    pin = r.Ledger(host.root, 'pin', pin_inputs['binding'])
    r.require(list(pin.completed()) == list(PIN_ACTIONS) and pin.completed()['complete']['passed'] is True,
              'separate pin primitive is not complete')
    parent_identity = host.owner['coordinator_identity']
    r.require(host.executor_identity() == parent_identity, 'final observer is not original parent')
    executor_records(host.root, release)
    identities = collect_identities(host.root, parent_identity)
    host.io.observations.clear()
    before_host = host.io.host()
    roles = {role: host.inspect_role(role) for role in ('provider', 'reservoir')}
    r.require(all(not view['job_loaded'] and view['process'] is None and not view['listener_pids']
                  and view['held'] is None for view in roles.values()), 'owned jobs/holders remain')
    observed = {pid: host.io.process(int(pid)) for pid in sorted(identities, key=int)}
    r.require(all(value is None for value in observed.values()), 'owned PID survives or was reused')
    ports = sorted(set().union(*host.known_ports.values()))
    port_observations = {str(port): host.io.port_pids(port) for port in ports}
    r.require(all(not value for value in port_observations.values()), 'owned listener port remains occupied')
    sentinel_ref = r.reference(host.root / 'pin/sentinel-exit-intent.json')
    sentinel = r.check_reference(sentinel_ref)
    sentinel_process = host.io.process(sentinel['pid'])
    pin_observed = PinSystem(host).pin_observe(host.target('pin'), sentinel['pid'])
    r.require(sentinel_process is None and not pin_observed['job_loaded'] and pin_observed['process'] is None
              and pin_observed['recorded_pid_absent'] is True and not pin_observed['listener_pids'], 'pin job/PID remains')
    guards = {role: (host.control(role) / 'hold.json').exists() for role in ('provider', 'reservoir')}
    r.require(not any(guards.values()), 'owned hold guard remains')
    state = r.reference(host.root / 'state.json'); backup = context['main_backup']['evidence']['backup']
    r.require(r.check_reference(state) == STATE and r.check_reference(backup)['expected_state'] == BASELINE
              and r.check_reference(backup)['files']['state.json']['value'] == BASELINE, 'final/backup state differs')
    applied = [row for row in host.events('reservoir', complete=True) if row['phase'] == 'checkin_applied']
    r.require(len(applied) == 2 and len({row['request_id'] for row in applied}) == 2
              and [row['state']['fixture_astrid']['ticks'] for row in applied] == [11, 22], 'two check-ins not preserved')
    requests = [r.reference(path) for path in sorted(host.root.glob('request.*.json'))]
    acks = [r.reference(path) for path in sorted(host.root.glob('ack.*.json'))]
    r.require(len(requests) == len(acks) == 2, 'extra or missing check-in files')
    after_host = host.io.host()
    r.require(before_host == after_host == release['host_binding'], 'final host binding changed')
    queries = r.write(host.root / 'final-raw-queries.json', {'schema': 'owned_fixture_raw_queries_v2',
                      'host_binding': after_host, 'observer_identity': parent_identity,
                      'queries': list(host.io.observations), 'pin_query': pin_observed['raw']})
    return r.write(host.root / 'final-observations.json', {
        'schema': 'owned_fixture_final_observations_v2', 'host_binding': after_host,
        'observed_wall_time_ns': time.time_ns(), 'observer_identity': parent_identity,
        'roles': roles, 'known_identities': identities, 'pid_observations': observed,
        'sentinel': {'receipt': sentinel_ref, 'pid': sentinel['pid'], 'process': sentinel_process,
                     'pin_observation': pin_observed}, 'ports': port_observations, 'guards': guards,
        'state': state, 'main_backup': backup, 'checkin_requests': requests, 'checkin_acks': acks,
        'raw_queries': queries, 'parent_absence_claimed': False, 'force_used': False})


def refs(root, pattern):
    return [r.reference(path) for path in sorted(root.glob(pattern))]


def produce(host, core, release_ref, parent_actions):
    """Publish outside the run root; no self-reference or self-reap assertion."""
    from owned_fixture_coordinator import owned_release, fixture_origins
    from owned_recovery_core import loaded_origins
    release = owned_release(host, release_ref); root = host.root
    parent = r.Ledger(root, 'parent', {'release_sha256': release_ref['sha256'],
                      'parent_identity_sha256': r.digest(host.owner['coordinator_identity'])})
    r.require(list(parent.completed()) == parent_actions and not any(row['type'] == 'failed' for row in parent.rows),
              'parent finite plan incomplete or failed')
    executors = executor_records(root, release)
    artifacts, exclusions = inventory(root)
    from owned_fixture_commands import capture
    native=capture(release)
    value = {
        'schema': 'reservoir_owned_recovery_fixture_result_v2', 'scope': 'owned_fixture_only',
        'release': release_ref, 'root': str(root), 'transaction_id': host.token,
        'host_binding': host.host_binding, 'python': release['python'], 'python_executables': release['python_files'],
        'final_release_closure': release['core']['artifacts'], 'fixture_artifacts': release['artifacts'],
        'plan': r.reference(root / 'plan.json'), 'injections': release['injections'],
        'parent': {'identity': host.owner['coordinator_identity'], 'start': r.reference(root / 'parent-start.json'),
                   'header': r.reference(root / 'parent/header.json'), 'events': refs(root / 'parent', 'event-*.json'),
                   'head_sha256': parent.head, 'parent_reaped': False, 'post_terminal_closeout_required': True},
        'setup': {'owner': r.reference(root / 'setup-owner.json'), 'complete': r.reference(root / 'setup-complete.json'),
                  'header': r.reference(root / 'setup/header.json'), 'events': refs(root / 'setup', 'event-*.json'),
                  'host_operations': refs(root / 'setup-host', 'host-operation-*.json')},
        'executors': executors,
        'readiness':{'records':[ref for ref in refs(root/'readiness','*.json') if not ref['path'].endswith('-inspection.json')],
                     'inspections':refs(root/'readiness','*-inspection.json')},'native':native,
        'main': {'preparation': r.reference(root / 'preparation.json'), 'controller_plan': r.reference(root / 'controller/plan.json'),
                 'events': refs(root / 'controller', 'event-*.json'), 'host_operations': refs(root, 'host-operation-*.json'),
                 'witnesses': refs(root, '*-witness.json'), 'worker_logs': refs(root, '*.events.jsonl'),
                 'backup': r.reference(root / 'fixture-backup.json'), 'private_restore': r.reference(root / 'fixture-private-restore.json')},
        'recovery': {'header': r.reference(root / 'recovery/header.json'), 'events': refs(root / 'recovery', 'event-*.json'),
                     'sessions': refs(root / 'execution-sessions', '*.json')},
        'cleanup': {'context': r.reference(root / 'cleanup/context.json'), 'header': r.reference(root / 'cleanup/header.json'),
                    'events': refs(root / 'cleanup', 'event-*.json'), 'host_operations': refs(root / 'cleanup', 'host-operation-*.json'),
                    'witnesses': refs(root / 'cleanup', '*-witness.json')},
        'pin': {'inputs': r.reference(root / 'pin/inputs.json'), 'header': r.reference(root / 'pin/header.json'),
                'events': refs(root / 'pin', 'event-*.json'), 'sentinel': r.reference(root / 'pin/sentinel-exit-intent.json')},
        'final_observations': r.reference(root / 'final-observations.json'), 'artifacts': artifacts,
        'excluded_lock_paths': exclusions, 'core_loaded_origins': loaded_origins(core), 'fixture_loaded_origins': fixture_origins(),
        'started_wall_time_ns': r.read(root / 'parent-start.json')['started_wall_time_ns'], 'completed_wall_time_ns': time.time_ns(),
        'exact_release_fixture_passed': True, 'production_qualified': False, 'production_host_executed': False,
        'production_pin_schedule_executed': False, 'numeric_restore_executed': False}
    r.require(inventory(root) == (artifacts, exclusions), 'run artifacts changed during aggregate capture')
    destination = Path(release['result_path'])
    if not destination.parent.exists(): destination.parent.mkdir(mode=0o700)
    return r.write(destination, value)
