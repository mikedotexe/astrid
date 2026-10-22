"""Finite owned executors for the exact final05 closure; disabled during development."""
from __future__ import annotations

import argparse
from contextlib import nullcontext
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from owned_fixture_records import (check_reference, digest, directory, identity, raw, read,
                                   reference, require, write)

HERE = Path(__file__).resolve().parent
RUNS = HERE / 'runs'
FIXTURE_EXECUTION_ENABLED = False
MAX_STEPS = 96
CONTROL = Path('/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/steward_control_v1/control.json')
LEASE = CONTROL.with_name('lease.json')


def fixture_origins():
    result = {}
    for module in tuple(sys.modules.values()):
        filename = getattr(module, '__file__', None)
        if filename is None:
            continue
        path = Path(filename)
        if path.parent == HERE:
            require(path.suffix == '.py' and path.resolve() == path, 'fixture module origin alias differs')
            result[str(path)] = reference(path)['sha256']
    return result


def validate_python(release):
    """Bind the selected launcher and exact macOS framework re-exec family."""
    supplied = Path(release['python'])
    require(supplied.is_absolute(), 'selected interpreter must be absolute')
    resolved = supplied.resolve(strict=True); paths = {resolved}
    # Same narrow rule as the reviewed hold helper's interpreter_paths().
    if (resolved.parent.name == 'bin' and resolved.parent.parent.parent.name == 'Versions'
            and resolved.parent.parent.parent.parent.name == 'Python.framework'):
        runtime = resolved.parent.parent / 'Resources/Python.app/Contents/MacOS/Python'
        require(runtime.is_file(), 'framework Python runtime executable is missing')
        paths.add(runtime.resolve(strict=True))
    require(isinstance(release.get('python_files'), dict)
            and set(release['python_files']) == {str(p) for p in paths},
            'selected interpreter does not match reviewed family')
    for path in paths:
        require(reference(path)['sha256'] == release['python_files'][str(path)], 'reviewed interpreter changed')
    require(supplied.resolve(strict=True) == resolved, 'selected interpreter alias changed')


def validate_coordination(release):
    """Read the one existing pause record; no lease expiry, repair or mutation."""
    value = release.get('coordination')
    require(isinstance(value, dict) and set(value) == {'control', 'lease_path', 'paused',
            'pause_generation', 'actor', 'lease_absent'} and isinstance(value['control'], dict)
            and value['control'].get('path') == str(CONTROL) and value['lease_path'] == str(LEASE)
            and value['paused'] is True and type(value['pause_generation']) is int
            and value['pause_generation'] == 528 and value['actor'] == 'codex-mlx-upgrade'
            and value['lease_absent'] is True, 'fixed steward coordination binding differs')
    require(not LEASE.exists() and not LEASE.is_symlink(), 'steward lease exists; no fixture action permitted')
    state = check_reference(value['control'])
    require(state.get('schema') == 'steward_control_state_v1' and state.get('schema_version') == 1
            and state.get('paused') is True and type(state.get('pause_generation')) is int
            and state['pause_generation'] == 528 and state.get('actor') == 'codex-mlx-upgrade',
            'current steward pause ownership differs')
    require(not LEASE.exists() and not LEASE.is_symlink(), 'steward lease appeared during coordination check')
    return {'coordination': value, 'observed_wall_time_ns': time.time_ns()}


def validate_release(path):
    release = read(path)
    require(release.get('schema') == 'owned_recovery_fixture_release_v2'
            and release.get('approved') is True and release.get('scope') == 'owned_fixture_only'
            and release.get('production_qualified') is False and release.get('final_release_acceptance') is False,
            'exact final owned release missing')
    validate_coordination(release)
    from owned_recovery_host import WORKER_SOURCE, HOLD_SOURCE
    from owned_recovery_core import validate_core
    from owned_fixture_plan import proposed_schedule
    expected = {str(p) for p in HERE.glob('*.py')} | {str(HERE / 'README.md'), str(WORKER_SOURCE), str(HOLD_SOURCE)}
    require(expected == set(release.get('artifacts', {})), 'exact fixture source/helper closure differs')
    require(release.get('qualification_meaning') == 'owned_fixture_exact_final_release_only'
            and release.get('release_stage') == 'final_enabled', 'final enabled release required')
    validate_core(release.get('core', {}), enabled=True)
    for filename, wanted in release['artifacts'].items():
        require(reference(filename)['sha256'] == wanted, 'reviewed source/helper changed')
    validate_python(release)
    fixture = release['fixture']; root = Path(fixture['root'])
    from owned_fixture_records import target
    require(root.parent == RUNS and root.name == 'owned-recovery-' + fixture['token']
            and fixture['targets'] == {role: target(release['host_binding']['uid'], fixture['token'], role)
                                       for role in ('provider', 'reservoir', 'pin')}, 'unexpected fixture destination')
    require(release.get('result_path') == str(HERE / 'evidence' / (root.name + '-result.json')),
            'fixed exclusive result destination required')
    require(release.get('sudo_debug') is True and release.get('setup_plan') == ['bootstrap:reservoir', 'bootstrap:provider'],
            'ordinary-user Terminal debug and exact setup plan required')
    schedule = proposed_schedule()
    require(release.get('executor_plan') == schedule['executor_plan']
            and len(schedule['executor_plan']) == 71
            and check_reference(release['injections']) == schedule['injections'], 'sealed71 schedule/injections differ')
    source_review = check_reference(release['source_review'])
    require(source_review.get('schema') == 'owned_recovery_fixture_final_release_source_review_v2'
            and source_review.get('approved') is True and source_review.get('scope') == 'owned_fixture_only'
            and bool(source_review.get('reviewer')) and source_review.get('release_stage') == 'final_enabled'
            and source_review.get('fixture_artifacts') == release['artifacts']
            and source_review.get('core_artifacts') == release['core']['artifacts']
            and source_review.get('core_flags') == release['core']['flags']
            and source_review.get('python') == release['python']
            and source_review.get('python_files') == release['python_files']
            and source_review.get('coordination') == release['coordination']
            and source_review.get('executor_plan_sha256') == digest(schedule['executor_plan'])
            and source_review.get('injections_sha256') == digest(schedule['injections']),
            'exact final source/interpreter/schedule review missing')
    steps = release.get('executor_plan')
    require(isinstance(steps, list) and 0 < len(steps) <= MAX_STEPS
            and len({row['id'] for row in steps}) == len(steps), 'finite unique executor plan required')
    for row in steps:
        require(set(row) == {'id', 'kind', 'mode', 'action_id', 'injection', 'expected_exit'}
                and re.fullmatch('[a-z0-9][a-z0-9-]{0,47}', row['id'])
                and row['kind'] in ('fresh', 'resume_one', 'cleanup', 'pin', 'lock_hold', 'lock_refusal')
                and row['mode'] in ('execute', 'reconcile')
                and row['expected_exit'] in (0, 75, 76), 'invalid finite executor step')
    for index, row in enumerate(steps):
        if row['kind'] == 'lock_hold':
            require(index + 1 < len(steps) and steps[index + 1]['kind'] == 'lock_refusal'
                    and row['mode'] == 'execute' and row['injection'] is None and row['expected_exit'] == 0,
                    'lock holder must be a finite nonmutating adjacent probe')
        if row['kind'] == 'lock_refusal':
            require(index > 0 and steps[index - 1]['kind'] == 'lock_hold'
                    and row['mode'] == 'execute' and row['injection'] is None and row['expected_exit'] == 76,
                    'lock refusal must follow its sole nonmutating holder')
    check_reference(release['injections'])
    return release


def owned_release(host, release_ref):
    """No alternate self-consistent release can select an executor launcher."""
    host.check_owned()
    require(release_ref == host.owner['release_ref'], 'executor release differs from retained owner')
    release = validate_release(release_ref['path'])
    require(reference(release_ref['path']) == release_ref
            and release['fixture']['root'] == str(host.root)
            and release['python'] == str(host.executable), 'owned executor release changed')
    return release


def prepare_executor(root, release_ref, step_id, host, *, expected_heads):
    """Create exactly one new planned request. Previous child must be reaped."""
    root = directory(root); release = owned_release(host, release_ref)
    require(reference(release_ref['path']) == release_ref and release['fixture']['root'] == str(root), 'release changed')
    require(read(root / 'setup-complete.json')['plan_ref'] == reference(root / 'plan.json'), 'setup incomplete')
    host.check_owned()
    require(host.executor_identity() == host.owner['coordinator_identity'], 'only the original live parent prepares executors')
    plan = release['executor_plan']; matches = [i for i, row in enumerate(plan) if row['id'] == step_id]
    require(len(matches) == 1, 'unplanned executor step'); index = matches[0]
    parent = root / 'executors'
    if not parent.exists(): parent.mkdir(mode=0o700)
    directory(parent)
    previous_steps = []
    for prior_index, prior in enumerate(plan[:index]):
        if (plan[index]['kind'] == 'lock_refusal' and prior['kind'] == 'lock_hold'
                and prior_index == index - 1):
            prior_root = parent / prior['id']; held = read(prior_root / 'lock-held.json')
            started = read(prior_root / 'self-start.json'); launched = read(prior_root / 'launch.json')
            require(held['request'] == reference(prior_root / 'request.json')
                    and started['request'] == held['request'] == launched['request']
                    and started['identity'] == held['identity']
                    and host.io.process(held['identity']['pid']) == held['identity'],
                    'dedicated no-action lock holder is not exactly current')
            previous_steps.append({'kind': 'live_nonmutating_lock_holder', 'step_id': prior['id'],
                  'held': reference(prior_root / 'lock-held.json'),
                  'start': reference(prior_root / 'self-start.json'), 'launch': reference(prior_root / 'launch.json')})
            continue
        prior_root = parent / prior['id']; exit_record = read(prior_root / 'exit.json')
        require(exit_record.get('reaped') is True and exit_record.get('returncode') == prior['expected_exit']
                and exit_record.get('timed_out') is False and not (prior_root / 'unresolved.json').exists(),
                'previous executor not naturally reaped as planned')
        identity_record = read(prior_root / 'self-start.json')['identity']
        require(host.io.process(identity_record['pid']) is None, 'previous executor remains alive')
        previous_steps.append({'kind': 'naturally_reaped_executor', 'step_id': prior['id'],
                               'exit': reference(prior_root / 'exit.json')})
    path = parent / step_id
    require(path.parent == parent and path.parent.resolve() == parent, 'executor step escaped private directory')
    path.mkdir(mode=0o700)  # Existing attempted or merely prepared request never authorizes replacement.
    request = {'schema': 'owned_fixture_executor_request_v1', 'step': plan[index], 'release': release_ref,
               'root': str(root), 'expected_heads': expected_heads, 'coordinator_identity': host.executor_identity(),
               'previous_steps': previous_steps}
    write(path / 'request.json', request)
    return path


def initialize_fixture(release_path, *, initial_identity, initial_host_binding, started_wall_time_ns,
                       initial_raw_queries):
    """Explicit finite setup; actual Host rechecks caller observations before launch."""
    require(FIXTURE_EXECUTION_ENABLED is True, 'owned fixture OS execution disabled in preparatory source')
    from owned_recovery_core import load_core
    from owned_recovery_host import FixtureHost, WORKER_SOURCE, HOLD_SOURCE
    from owned_fixture_system import OwnedSystem
    from owned_fixture_setup import create_layout, copy_exclusive, retain_plists, bootstrap_one, publish_complete
    release = validate_release(release_path); core = load_core(release)
    root = Path(release['fixture']['root']); token = release['fixture']['token']
    if not RUNS.exists(): RUNS.mkdir(mode=0o700)
    require(initial_host_binding == release['host_binding'], 'setup host is not reviewed host')
    create_layout(root, token, reference(release_path), initial_identity, initial_host_binding,
                  release['python'], WORKER_SOURCE, HOLD_SOURCE)
    from owned_fixture_records import Ledger
    write(root / 'parent-start.json', {'schema': 'owned_fixture_parent_start_v2', 'identity': initial_identity,
          'host_binding': initial_host_binding, 'release': reference(release_path),
          'started_wall_time_ns': started_wall_time_ns, 'parent_reaped': False,
          'raw_queries': initial_raw_queries, 'coordination_observation': validate_coordination(release)})
    ledger = Ledger(root, 'parent', {'release_sha256': reference(release_path)['sha256'],
                                    'parent_identity_sha256': digest(initial_identity)}, create=True)
    claim = ledger.append('claim', 'setup', executor_identity=initial_identity)
    try:
        result = _finish_setup(root, token, release, core, initial_identity, initial_host_binding)
        ledger.append('result', 'setup', claim_sha256=digest(claim), result=result)
        return result
    except BaseException as error:
        ledger.append('failed', 'setup', claim_sha256=digest(claim), error_type=type(error).__name__,
                      error=str(error)[:1000], retry_permitted=False)
        raise


def _finish_setup(root, token, release, core, initial_identity, initial_host_binding):
    from owned_recovery_host import FixtureHost
    from owned_fixture_system import OwnedSystem
    from owned_fixture_setup import copy_exclusive, retain_plists, bootstrap_one, publish_complete
    for name in ('owned_pin_sentinel.py', 'owned_fixture_records.py'):
        require(release['artifacts'].get(str(HERE / name)) == reference(HERE / name)['sha256'], 'unreviewed pin helper')
        copy_exclusive(HERE / name, root / name)
    io = OwnedSystem(root, token, release['python'])
    host = FixtureHost.create(root, token, release['python'], release['artifacts'],
                             core=core, io=io, sudo_debug=bool(release.get('sudo_debug')))
    require(host.executor_identity() == initial_identity and io.host() == initial_host_binding,
            'setup identity is not actual coordinator/host')
    retain_plists(root, host)
    originals = {}
    for role in ('reservoir', 'provider'):
        validate_coordination(release)
        originals[role] = bootstrap_one(root, host, role, mode='execute', executor=host.executor_identity())['identity']
    plan = core['m'].validate_plan({'schema': 'reservoir_maintenance_plan_v1', 'transaction_id': token,
        'actor': 'owned-fixture-only', 'pause_generation': 1, 'host_binding': host.host_binding,
        'input_hashes': host.inputs(), 'roles': {role: {'label': core['m'].LABELS[role], 'port': core['m'].PORTS[role],
          'identity': originals[role]} for role in ('provider', 'reservoir')}, 'protected': {}, 'timeout_s': 90})
    result = publish_complete(root, host, plan, host.specification(plan))
    return {'root': str(root), 'setup_complete': reference(root / 'setup-complete.json'),
            'scope': 'owned_fixture_only', 'production_qualified': False}


def run_lock_pair(root, release_ref, holder_step, refusal_step, host):
    """Only this adjacent no-service pair may overlap; both natural reaps required."""
    require(FIXTURE_EXECUTION_ENABLED is True, 'owned fixture OS execution disabled in preparatory source')
    release = owned_release(host, release_ref); plan = release['executor_plan']
    index = next((i for i, row in enumerate(plan) if row['id'] == holder_step), -1)
    require(index >= 0 and index + 1 < len(plan) and plan[index]['kind'] == 'lock_hold'
            and plan[index + 1]['id'] == refusal_step and plan[index + 1]['kind'] == 'lock_refusal',
            'only reviewed adjacent lock pair supported')
    holder = prepare_executor(root, release_ref, holder_step, host, expected_heads={})
    request = read(holder / 'request.json')
    argv = [release['python'], '-B', str(HERE / 'owned_fixture_coordinator.py'),
            '--executor-request', str(holder / 'request.json')]
    write(holder / 'launch.json', {'schema': 'owned_fixture_executor_launch_v1', 'argv': argv,
           'request': reference(holder / 'request.json'), 'release': release_ref,
           'coordinator_identity': host.executor_identity(), 'started_wall_time_ns': time.time_ns(),
           'timeout_s': 90, 'automatic_retry_permitted': False, 'service_actions_permitted': False})
    process = None
    try:
        with (holder / 'stdout').open('xb') as out, (holder / 'stderr').open('xb') as err:
            owned_release(host, release_ref)
            process = subprocess.Popen(argv, stdin=None, stdout=out, stderr=err, cwd=str(host.root))
            write(holder / 'child-created.json', {'pid': process.pid, 'argv': argv, 'launch': reference(holder / 'launch.json')})
            deadline = time.monotonic() + 30
            while not (holder / 'lock-held.json').exists():
                require(process.poll() is None and time.monotonic() < deadline, 'owned lock holder failed to become ready')
                time.sleep(.1)
            held = read(holder / 'lock-held.json')
            require(held['identity']['pid'] == process.pid and held['request'] == reference(holder / 'request.json')
                    and host.io.process(process.pid) == held['identity'], 'lock holder identity changed')
            refusal = prepare_executor(root, release_ref, refusal_step, host, expected_heads={})
            refused = launch_executor(refusal, host, timeout=30)
            require(refused['returncode'] == 76 and read(refusal / 'lock-refusal.json')['action_performed'] is False,
                    'competing caller did not refuse before action')
            write(holder / 'release-lock.json', {'holder_request': reference(holder / 'request.json'),
                                                'refusal_exit': reference(refusal / 'exit.json')})
            code = process.wait(timeout=30)
        started = read(holder / 'self-start.json')
        require(started['identity'] == held['identity'] and host.env.command_matches(started['identity']['argv'], argv),
                'lock holder startup differs')
        result = {'schema': 'owned_fixture_executor_exit_v1', 'start': started, 'pid': process.pid,
                  'returncode': code, 'reaped': True, 'timed_out': False, 'signals_sent': 0,
                  'completed_wall_time_ns': time.time_ns(), 'stdout': reference(holder / 'stdout'),
                  'stderr': reference(holder / 'stderr')}
        write(holder / 'exit.json', result)
        require(code == 0 and host.io.process(process.pid) is None, 'lock holder natural exit differs')
        return {'holder': reference(holder / 'exit.json'), 'refusal': reference(refusal / 'exit.json'),
                'service_actions_performed': 0, 'production_qualified': False}
    except BaseException as error:
        write(holder / 'unresolved.json', {'error_type': type(error).__name__, 'error': str(error)[:1000],
            'pid': None if process is None else process.pid, 'force_used': False, 'automatic_retry_permitted': False})
        raise


def launch_executor(executor_root, host, *, timeout=300, spawn=None):
    """Single owned Popen. Timeout records unresolved state and never signals."""
    require(FIXTURE_EXECUTION_ENABLED is True, 'owned fixture OS execution disabled in preparatory source')
    require(type(timeout) is int and 1 <= timeout <= 1200, 'bounded executor deadline required')
    executor_root = directory(executor_root); request = read(executor_root / 'request.json')
    release = owned_release(host, request['release'])
    require(Path(request['root']) == host.root and executor_root.parent == host.root / 'executors', 'foreign executor root')
    require(not (executor_root / 'launch.json').exists(), 'executor was attempted; never relaunch')
    argv = [release['python'], '-B', str(HERE / 'owned_fixture_coordinator.py'),
            '--executor-request', str(executor_root / 'request.json')]
    start = {'schema': 'owned_fixture_executor_launch_v1', 'argv': argv, 'request': reference(executor_root / 'request.json'),
             'release': request['release'], 'coordinator_identity': host.executor_identity(),
             'started_wall_time_ns': time.time_ns(), 'timeout_s': timeout, 'automatic_retry_permitted': False}
    write(executor_root / 'launch.json', start)
    process = None
    try:
        with (executor_root / 'stdout').open('xb') as out, (executor_root / 'stderr').open('xb') as err:
            owned_release(host, request['release'])
            process = (spawn or subprocess.Popen)(argv, stdin=None, stdout=out, stderr=err, cwd=str(host.root))
            write(executor_root / 'child-created.json', {'pid': process.pid, 'argv': argv, 'launch': reference(executor_root / 'launch.json')})
            code = process.wait(timeout=timeout)
        child_start = read(executor_root / 'self-start.json'); identity(child_start['identity'])
        require(child_start['identity']['pid'] == process.pid and child_start['request'] == start['request']
                and host.env.command_matches(child_start['identity']['argv'], argv), 'executor startup differs')
        result = {'schema': 'owned_fixture_executor_exit_v1', 'start': child_start, 'pid': process.pid,
                  'returncode': code, 'reaped': True, 'timed_out': False, 'signals_sent': 0,
                  'completed_wall_time_ns': time.time_ns(), 'stdout': reference(executor_root / 'stdout'),
                  'stderr': reference(executor_root / 'stderr')}
        write(executor_root / 'exit.json', result)
        require(code == request['step']['expected_exit'], 'executor exit differs from finite plan')
        if code == 75:
            from owned_fixture_injection import verify_executor_exit, validate_manifest
            injections = validate_manifest(check_reference(release['injections']))
            point = next(row for row in injections['points'] if row['id'] == request['step']['injection'])
            verify_executor_exit(child_start, result, read(executor_root / 'injection-ack.json'), point, request['release'])
        host.check_owned()
        return result
    except BaseException as error:
        write(executor_root / 'unresolved.json', {'schema': 'owned_fixture_executor_unresolved_v1',
              'pid': None if process is None else process.pid, 'error_type': type(error).__name__,
              'error': str(error)[:1000], 'force_used': False, 'automatic_retry_permitted': False,
              'running_child_absence_inferred': False})
        raise


def execute_request(path, *, host_factory=None, system_factory=None):
    require(FIXTURE_EXECUTION_ENABLED is True, 'owned fixture OS execution disabled in preparatory source')
    from owned_recovery_core import load_core
    from owned_recovery_host import FixtureHost
    from owned_fixture_system import OwnedSystem
    from owned_fixture_injection import Injection, wrap_core
    request = read(path); executor_root = Path(path).parent
    require(read(Path(request['root']) / 'setup-owner.json')['release_ref'] == request['release'],
            'executor request release differs from retained owner')
    release = validate_release(request['release']['path']); core = load_core(release)
    io = (system_factory or OwnedSystem)(Path(request['root']), release['fixture']['token'], release['python'])
    host = (host_factory or FixtureHost).attach(Path(request['root']), io=io,
            sudo_debug=bool(release.get('sudo_debug')), core=core)
    release = owned_release(host, request['release'])
    require(executor_root == host.root / 'executors' / request['step']['id']
            and Path(path) == executor_root / 'request.json'
            and request['coordinator_identity'] == host.owner['coordinator_identity']
            and io.process(request['coordinator_identity']['pid']) == request['coordinator_identity'],
            'executor request lacks its exact live parent/path')
    actual = identity(host.executor_identity()); require(actual['pid'] == os.getpid(), 'wrong executor process')
    from owned_recovery_core import loaded_origins
    write(executor_root / 'self-start.json', {'identity': actual, 'request': reference(path),
            'release': request['release'], 'started_wall_time_ns': time.time_ns(),
            'coordination_observation': validate_coordination(release),
            'core_loaded_origins': loaded_origins(core), 'fixture_loaded_origins': fixture_origins()})
    step = request['step']; require(step in release['executor_plan'], 'unplanned executor command')
    injection = Injection(host, release['injections'], step['injection'], executor_root) if step['injection'] else None
    with wrap_core(core, injection) if injection else nullcontext():
        if step['kind'] == 'fresh':
            require(not (host.root / 'controller').exists(), 'fresh main journal already exists')
            journal = core['m'].Journal(host.root / 'controller', plan=host.plan)
            result = core['m'].Controller(journal, host).run()
        elif step['kind'] == 'resume_one':
            journal = core['m'].Journal(host.root / 'controller')
            report = host.recovery_inspection(journal)
            ledger = core['recovery_ledger'].read_ledger(host.root, host.plan)
            checkpoint = core['recovery_phases'].checkpoint_from_inspection(host.spec, report, ledger)
            require(checkpoint['action_id'] == step['action_id']
                    and core['production_recovery_driver'].expected_heads(checkpoint) == request['expected_heads'],
                    'finite requested recovery phase/head differs before action')
            host.expect_recovery_action(step['action_id'])
            result = core['production_recovery_driver'].resume_one(journal, host,
                     expected=request['expected_heads'], mode=step['mode'])
            require(result['action_id'] == step['action_id'], 'continued wrong finite phase')
        elif step['kind'] == 'cleanup':
            from owned_fixture_cleanup import step as cleanup_step
            result = cleanup_step(host, core, expected_head=request['expected_heads']['cleanup'],
                                  action=step['action_id'], mode=step['mode'])
        elif step['kind'] == 'pin':
            from owned_fixture_pin import step as pin_step
            from owned_pin_system import PinSystem
            result = pin_step(host, core, PinSystem(host), expected_head=request['expected_heads']['pin'],
                              action=step['action_id'], mode=step['mode'],
                              boundary_hook=None if injection is None else lambda name, ref: injection.observe('pin', name, ref))
        elif step['kind'] == 'lock_hold':
            require(step['injection'] is None, 'lock holder cannot inject or control jobs')
            with core['transaction_execution'].ExecutionSession(host.root, host.plan, actual, 'resume_one') as session:
                write(executor_root / 'lock-held.json', {'identity': actual, 'request': reference(path),
                        'session_begin': reference(session.begin_path), 'service_actions_permitted': False})
                deadline = time.monotonic() + 60
                while not (executor_root / 'release-lock.json').exists():
                    require(time.monotonic() < deadline, 'lock probe timed out; natural exit, no force')
                    time.sleep(.1)
                release_lock = read(executor_root / 'release-lock.json')
                require(release_lock['holder_request'] == reference(path)
                        and check_reference(release_lock['refusal_exit'])['returncode'] == 76,
                        'lock release does not name accepted owned refusal')
                return 0
        else:
            # A concurrently held lock must refuse before any Journal/Host action.
            try:
                with core['transaction_execution'].ExecutionSession(host.root, host.plan, actual, 'resume_one'):
                    raise ValueError('concurrent lock unexpectedly acquired')
            except core['m'].Refusal as error:
                require('transaction already executing' in str(error), 'unexpected lock-refusal reason')
                write(executor_root / 'lock-refusal.json', {'passed': True, 'reason': str(error), 'action_performed': False})
                return 76
        require(injection is None, 'planned injection boundary was not reached')
        write(executor_root / 'result.json', {'result': result, 'production_qualified': False,
              'final_release_acceptance': False, 'core_loaded_origins': loaded_origins(core),
              'fixture_loaded_origins': fixture_origins()})
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--executor-request', type=Path, required=True)
    args = parser.parse_args()
    return execute_request(args.executor_request)


if __name__ == '__main__': raise SystemExit(main())
