#!/usr/bin/env python3
"""Reviewed, explicitly invoked owned launchd fixture; never a production host.

Failures retain jobs, holds and evidence. Only exact verified stdlib holders may
be booted out. Fake-worker evidence does not qualify production state restore.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import re
import shlex
import signal
import subprocess
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
HOLD_SOURCE = Path('/Users/v/other/astrid/scripts/model_environment.py')
EVIDENCE_ROOT = HERE.parent / 'evidence'
PREFIX = 'com.codex.reservoir-maintenance-fixture.'
OWNER_SCHEMA = 'reservoir_maintenance_owned_fixture_v1'
FORBIDDEN_PORTS = {7878, 7879, 7880, 7881, 8090, 8092, 17881, 11434}


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


m = load_module('owned_maintenance_controller', HERE / 'reservoir_maintenance.py')


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def required_artifacts():
    return [Path(__file__).resolve(), HERE / 'reservoir_maintenance.py',
            HERE / 'maintenance_fixture_worker.py', HERE / 'test_qualify_reservoir_maintenance.py',
            HERE / 'test_reservoir_maintenance.py', HERE / 'test_maintenance_fixture_worker.py', HOLD_SOURCE]


def verify_review(path):
    review = m.read_record(path)
    m.require(review.get('schema') == 'reservoir_maintenance_fixture_review_v1'
              and review.get('approved') is True, 'fixture has no accepted source review')
    expected = {str(item): sha(item) for item in required_artifacts()}
    m.require(review.get('artifacts') == expected, 'reviewed helper bytes changed or closure incomplete')
    cpu = review.get('cpu_receipt', {})
    m.require(isinstance(cpu.get('path'), str) and sha(cpu['path']) == cpu.get('sha256'),
              'CPU qualification receipt is missing or changed')
    qualification = m.read_record(Path(cpu['path']))
    m.require(qualification.get('passed') is True and
              all(qualification.get('artifacts', {}).get(path) == value for path, value in expected.items()),
              'CPU qualification does not bind the reviewed helper closure')
    return expected


def fixture_target(uid, token, role):
    m.require(type(uid) is int and uid > 0 and re.fullmatch(r'[0-9a-f]{32}', token)
              and role in m.ROLES, 'invalid owned fixture target')
    return f'gui/{uid}/{PREFIX}{token}.{role}'


def validate_target(target, uid, token, role):
    m.require(target == fixture_target(uid, token, role), 'target is outside exact owned fixture')


def debug_command(target, uid, token, role, arguments, *, sudo_debug):
    validate_target(target, uid, token, role)
    m.require(os.geteuid() != 0 and uid == os.getuid(), 'never sudo the whole fixture')
    command = ['/bin/launchctl', 'debug', target, *arguments]
    if sudo_debug:
        m.require(sys.stdin.isatty(), '--sudo-debug requires an interactive Terminal')
        command = ['/usr/bin/sudo', '--', *command]
    return command


def command(*args, check=True):
    return subprocess.run(args, capture_output=True, text=True, check=check, timeout=20)


def wait_for(predicate, description, timeout=60):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(.1)
    raise m.Refusal('timed out without escalation: ' + description)


class FixtureHost:
    scope = 'owned_fixture'

    def __init__(self, root, token, executable, artifacts, *, sudo_debug):
        self.root, self.token = Path(root), token
        self.executable, self.artifacts = Path(executable), dict(artifacts)
        self.sudo_debug, self.uid = sudo_debug, os.getuid()
        self.env = load_module('owned_fixture_hold', self.root / 'model_environment.py')
        self.worker = self.root / 'maintenance_fixture_worker.py'
        self.plan, self.guards, self.signalled, self.ports = None, {}, set(), {}
        self.armed = set()
        self.operations = 0
        self.host_binding = {'uid': self.uid, 'boot_uuid': command('/usr/sbin/sysctl', '-n', 'kern.bootsessionuuid').stdout.strip(),
                             'hostname': os.uname().nodename,
                             'os_version': command('/usr/bin/sw_vers', '-productVersion').stdout.strip()}
        self.check_owned()

    def check_owned(self):
        m.require(self.root == self.root.resolve(strict=True) and self.root.stat().st_uid == self.uid,
                  'fixture root is not canonical and owned')
        marker = m.read_record(self.root / 'fixture.owner.json')
        m.require(marker == {'schema': OWNER_SCHEMA, 'token': self.token, 'root': str(self.root), 'uid': self.uid},
                  'fixture ownership changed')
        m.require(self.uid > 0 and os.geteuid() != 0, 'ordinary user required')
        for path, digest in self.artifacts.items():
            m.require(sha(path) == digest, 'reviewed helper changed during fixture')
        for original, copied in ((HOLD_SOURCE, self.root / 'model_environment.py'),
                                 (HERE / 'maintenance_fixture_worker.py', self.worker)):
            m.require(sha(copied) == self.artifacts[str(original)], 'copied helper bytes changed')

    def target(self, role):
        return fixture_target(self.uid, self.token, role)

    def control(self, role):
        m.require(role in m.ROLES, 'unknown fixture role')
        return self.root / ('hold-' + role)

    def argv(self, role):
        return [str(self.executable), str(self.worker), '--root', str(self.root), '--token', self.token,
                '--role', role, '--drain-seconds', '6.0']

    def hold_argv(self, role):
        return [str(self.executable), str(self.root / 'model_environment.py'), 'launch',
                '--control-root', str(self.control(role)), '--hold-only', self.token]

    def plist_settings(self, role):
        return {'Label': self.target(role).split('/')[-1], 'ProgramArguments': self.argv(role),
                'WorkingDirectory': str(self.root), 'RunAtLoad': True, 'KeepAlive': True,
                'ThrottleInterval': 1, 'ExitTimeOut': 5,
                'StandardOutPath': str(self.root / f'{role}.stdout'),
                'StandardErrorPath': str(self.root / f'{role}.stderr')}

    def operation(self, name, **details):
        self.operations += 1
        m.write_new(self.root / f'host-operation-{self.operations:05d}.json',
                    {'operation': name, 'monotonic': time.monotonic(), **details})

    def job(self, role):
        self.check_owned()
        result = command('/bin/launchctl', 'print', self.target(role), check=False)
        if result.returncode:
            # Only the documented not-found outcome is absence; permissions and
            # transport errors cannot be converted into a stopped-state barrier.
            output = result.stdout + result.stderr
            m.require('Could not find service' in output or 'Could not find specified service' in output,
                      'launchd inspection failed without proven absence')
            return False, None
        found = re.search(r'^\s*pid = (\d+)\s*$', result.stdout, re.MULTILINE)
        return True, int(found[1]) if found else None

    def process(self, pid):
        if pid is None:
            return None
        started = self.env.process_start(pid)
        if started is None:
            return None
        result = command('/bin/ps', '-ww', '-p', str(pid), '-o', 'command=', check=False)
        if result.returncode:
            return None
        argv = shlex.split(result.stdout.strip())
        m.require(self.env.process_start(pid) == started, 'process changed during inspection')
        return {'pid': pid, 'started_at': started, 'argv': argv}

    def listener_ports(self, pid):
        result = command('/usr/sbin/lsof', '-nP', '-a', '-p', str(pid), '-iTCP', '-sTCP:LISTEN', '-Fpn', check=False)
        m.require(result.returncode in (0, 1), 'listener inspection failed')
        m.require(not result.stderr.strip(), 'listener inspection emitted an error')
        found = []
        for line in result.stdout.splitlines():
            if line.startswith('p'):
                m.require(line == 'p' + str(pid), 'foreign listener PID')
            elif line.startswith('n'):
                match = re.fullmatch(r'n127\.0\.0\.1:(\d+)', line)
                m.require(match is not None, 'unexpected listener address')
                port = int(match[1])
                m.require(port not in FORBIDDEN_PORTS, 'fixture selected a protected port')
                found.append(port)
        return found

    def inspect_role(self, role):
        loaded, pid = self.job(role)
        process = self.process(pid)
        view = {'target': self.target(role), 'job_loaded': loaded, 'process': process,
                'listener_pids': [], 'held': None}
        # Check every previously observed sentinel port, including after the old
        # PID exits. A foreign listener on it prevents a false absence claim.
        if role in self.ports:
            result = command('/usr/sbin/lsof', '-nP', f'-iTCP:{self.ports[role]}', '-sTCP:LISTEN', '-Fp', check=False)
            m.require(result.returncode in (0, 1) and not result.stderr.strip(), 'port ownership query failed')
            view['listener_pids'] = sorted({int(line[1:]) for line in result.stdout.splitlines() if line.startswith('p')})
        if process is None:
            return view
        ports = self.listener_ports(pid)
        if self.env.command_matches(process['argv'], self.argv(role)):
            ready_path = self.root / f'{role}.ready.json'
            if ready_path.exists():
                ready = m.read_record(ready_path)
                m.require(ready.get('token') == self.token and ready.get('role') == role
                          and ready.get('pid') == pid and ready.get('protocol') == 'fixture_sentinel_only',
                          'fixture ready identity mismatch')
                draining = (role, pid, process['started_at']) in self.signalled
                m.require(ports == [ready.get('port')] or (draining and not ports),
                          'fixture sentinel listener mismatch')
                self.ports[role] = ready['port']
                view['listener_pids'] = [pid] if ports else []
            else:
                # The worker opens its private sentinel before publishing its
                # ready receipt. Wait for that receipt and restored/idle event.
                m.require(len(ports) <= 1, 'worker has unexpected extra listeners')
            return view
        m.require(self.env.command_matches(process['argv'], self.hold_argv(role)), 'unexpected fixture process command')
        m.require(not ports and not view['listener_pids'], 'holding process has a listener')
        receipt_path = self.control(role) / 'held' / f'{pid}.json'
        if not receipt_path.exists():
            # The holder has started but has not published its first receipt.
            return dict(view, process=None)
        receipt = self.env.read_json(receipt_path)
        guard = self.guards.get(role)
        m.require(guard is not None and receipt.get('hold') == guard
                  and receipt.get('started_at') == process['started_at'] and receipt.get('hold_only') is True
                  and receipt.get('model_imported') is False and receipt.get('listener_opened') is False
                  and receipt.get('launcher_sha256') == sha(self.root / 'model_environment.py'), 'holder receipt mismatch')
        self.env.assert_hold(self.control(role), guard)
        view['held'] = {'guard': guard, 'identity': process, 'hold_only': True,
                        'stdlib_only': True, 'state_touched': False}
        return view

    def inputs(self):
        originals = {role: sha(self.root / f'{role}.plist') for role in m.ROLES}
        return {'provider_source': sha(self.worker), 'provider_settings': m.digest(originals['provider']),
                'provider_packages': m.digest('owned stdlib only, no installed model packages'),
                'model_snapshot': sha(self.root / 'model-never-opened.sentinel'),
                'reservoir_source': sha(self.root / 'reservoir-source.marker'),
                'reservoir_candidate': sha(self.root / 'reservoir-repair.marker'),
                'reservoir_settings': m.digest(originals['reservoir']),
                'interpreter': m.digest({str(path): sha(path) for path in self.env.interpreter_paths(self.executable)}),
                'helpers': m.digest(self.artifacts), 'fixture': self.artifacts[str(Path(__file__).resolve())]}

    def inspect(self):
        self.check_owned()
        return {'host_binding': self.host_binding, 'paused': True, 'pause_generation': 1,
                'pause_actor': 'owned-fixture-only', 'lease': None, 'input_hashes': self.inputs(), 'protected': {},
                'fixture_mode_authorized': True, 'originals_preserved': True, 'rollback_software_ready': True,
                'roles': {role: self.inspect_role(role) for role in m.ROLES},
                'ownership_scope': 'private fixture marker, not production stewardship'}

    def create_hold(self, role, guard):
        self.check_owned()
        m.require(guard.get('transaction_id') == self.token, 'hold is outside this fixture')
        result = self.env.create_hold(self.control(role), self.token, guard['actor'])
        m.require(result == guard, 'hold guard differs')
        self.guards[role] = result
        self.operation('hold_created', role=role, guard=guard)
        return {'guard': result}

    def arm_next_spawn(self, role, guard, expected):
        self.check_owned()
        self.env.assert_hold(self.control(role), guard)
        view = self.inspect_role(role)
        m.require(view['process'] == expected and view['held'] is None, 'worker identity changed before arm')
        arguments = ['--program', str(self.executable), '--', *self.hold_argv(role)[1:]]
        argv = debug_command(self.target(role), self.uid, self.token, role, arguments, sudo_debug=self.sudo_debug)
        self.operation('debug_intent', role=role, command=argv)
        if self.sudo_debug:
            result = subprocess.run(argv, check=False, timeout=180)
        else:
            result = command(*argv, check=False)
        self.operation('debug_result', role=role, returncode=result.returncode)
        m.require(result.returncode == 0, 'next-spawn override failed; worker not signalled')
        self.armed.add((role, expected['pid'], expected['started_at']))
        return {'armed': True, 'target': self.target(role), 'argument_contract': 'script_first_tail_host_qualified_only_by_this_fixture'}

    def signal_stop(self, role, expected):
        view = self.inspect_role(role)
        m.require(view['process'] == expected and view['held'] is None, 'wrong worker before cooperative signal')
        key = (role, expected['pid'], expected['started_at'])
        m.require(key in self.armed, 'successful owned next-spawn arm is required before signal')
        m.require(key not in self.signalled, 'never repeat a signal automatically')
        self.operation('signal_intent', role=role, identity=expected, signal='SIGTERM')
        self.signalled.add(key)
        os.kill(expected['pid'], signal.SIGTERM)
        self.operation('signal_sent', role=role, identity=expected)
        return {'signal': 'SIGTERM', 'identity': expected, 'force_used': False}

    def remove_held(self, role, expected, guard):
        view = self.inspect_role(role)
        m.require(view['process'] == expected and view['held'] is not None
                  and view['held']['guard'] == guard and not view['listener_pids'], 'bootout requires exact owned holder')
        self.operation('held_bootout_intent', role=role, identity=expected)
        command('/bin/launchctl', 'bootout', self.target(role))
        wait_for(lambda: not self.inspect_role(role)['job_loaded'] and self.env.process_start(expected['pid']) is None,
                 'verified held job absence', timeout=15)
        self.operation('held_job_absent', role=role, identity=expected)
        return {'removed_identity': expected}

    def events(self, role, pid):
        path = self.root / f'{role}.events.jsonl'
        if not path.exists():
            return []  # Process may exist before its first durable event.
        data = path.read_bytes()
        # A concurrently appended partial tail is not an event or readiness.
        # Once a worker exits, a missing final complete event fails the witness.
        complete = data[:data.rfind(b'\n') + 1]
        records = [json.loads(line) for line in complete.splitlines()]
        m.require(all(item.get('role') == role and item.get('token') == self.token for item in records), 'foreign worker event')
        return [item for item in records if item.get('pid') == pid]

    def collect_witness(self, kind, context):
        evidence = {'scope': 'owned_fixture_only', 'real_provider_or_persistence_proven': False}
        old = self.plan['roles']
        if kind == 'provider_drain':
            rows = self.events('provider', old['provider']['identity']['pid'])
            phases = [row['phase'] for row in rows]
            expected = ['started', 'idle', 'generation_admitted', 'stop_received', 'checkin_sent', 'checkin_acknowledged', 'drained']
            m.require(phases == expected, 'fake provider drain/check-in incomplete')
            request = rows[2]['request_id']
            m.require(all(row.get('request_id') == request for row in rows[2:]), 'fake request association differs')
            m.require(rows[5]['ticks'] == 11 and rows[-1]['monotonic'] - rows[3]['monotonic'] >= 6, 'fake drain did not exceed five seconds')
            m.require(self.inspect_role('reservoir')['process'] == old['reservoir']['identity'], 'reservoir changed during provider check-in')
            applied = [row for row in self.events('reservoir', old['reservoir']['identity']['pid'])
                       if row['phase'] == 'checkin_applied' and row.get('request_id') == request]
            m.require(len(applied) == 1, 'fake check-in not applied exactly once')
            evidence.update(events=rows, applied=applied)
        elif kind == 'final_snapshot':
            rows = self.events('reservoir', old['reservoir']['identity']['pid'])
            stops = [row for row in rows if row['phase'] == 'stop_received']
            finals = [row for row in rows if row['phase'] == 'final_snapshot_complete']
            m.require(len(stops) == len(finals) == 1 and finals[0]['monotonic'] - stops[0]['monotonic'] >= 6,
                      'fake final snapshot did not finish its long drain')
            saved = {row['name']: row['value'] for row in rows if row['phase'] == 'handle_saved'}
            expected = {'fixture_astrid': {'ticks': 11}, 'fixture_minime': {'ticks': 7}}
            m.require(saved == finals[0]['state'] == expected and rows[-1]['phase'] == 'drained', 'fake final state incomplete')
            m.require(m.read_record(self.root / 'state.json') == expected, 'fake saved bytes/state differ')
            evidence.update(final_handles=sorted(saved), state=expected, events=rows)
        elif kind == 'backup':
            m.require(all(not self.inspect_role(role)['job_loaded'] for role in m.ROLES), 'fake backup requires both jobs absent')
            data = (self.root / 'state.json').read_bytes()
            destination = self.root / 'preserved-state.json'
            m.write_new(destination, json.loads(data))
            m.require(m.read_record(destination) == json.loads(data), 'fake backup content differs')
            evidence.update(source_sha256=sha(self.root / 'state.json'), backup_sha256=sha(destination),
                            state=json.loads(data), all_files=True)
        elif kind == 'private_restore':
            restored = m.read_record(self.root / 'preserved-state.json')
            m.require(restored == {'fixture_astrid': {'ticks': 11}, 'fixture_minime': {'ticks': 7}}, 'fake restore differs')
            evidence.update(restored=restored, source_state_sha256=sha(self.root / 'state.json'))
        elif kind in ('startup', 'provider_ready'):
            role = 'reservoir' if kind == 'startup' else 'provider'
            view = self.inspect_role(role)
            m.require(view['process'] is not None and view['listener_pids'] == [view['process']['pid']], 'replacement not ready')
            rows = self.events(role, view['process']['pid'])
            if role == 'reservoir':
                restored = [row['state'] for row in rows if row['phase'] == 'restored']
                m.require(restored == [m.read_record(self.root / 'preserved-state.json')], 'replacement fake restore differs')
            else:
                m.require([row['phase'] for row in rows] == ['started', 'idle'], 'replacement provider not idle')
            evidence.update(identity=view['process'], events=rows)
        else:
            raise m.Refusal('unknown fixture witness kind')
        return {'schema': 'reservoir_maintenance_witness_v1', 'kind': kind, 'transaction_id': self.token,
                'passed': True, 'bindings': copy.deepcopy(context['bindings']), 'evidence': evidence}

    def install_repair(self, context):
        m.require(all(not self.inspect_role(role)['job_loaded'] for role in m.ROLES), 'repair requires fixture jobs absent')
        data = (self.root / 'reservoir-repair.marker').read_bytes()
        temporary = self.root / 'reservoir-source.next'
        with temporary.open('xb') as handle:
            handle.write(data); handle.flush(); os.fsync(handle.fileno())
        os.replace(temporary, self.root / 'reservoir-source.marker')
        m.sync_dir(self.root)
        return {'source_sha256': sha(self.root / 'reservoir-source.marker'), 'scope': 'synthetic marker only'}

    def bootstrap_original(self, role, context):
        m.require(not self.inspect_role(role)['job_loaded'], 'fixture job already loaded')
        m.require(plistlib.loads((self.root / f'{role}.plist').read_bytes()) == self.plist_settings(role),
                  'bootstrap plist is not the exact original private fixture configuration')
        self.operation('bootstrap_intent', role=role)
        command('/bin/launchctl', 'bootstrap', f'gui/{self.uid}', str(self.root / f'{role}.plist'))
        view = wait_for(lambda: self._ready_view(role), 'fixture replacement readiness')
        self.operation('bootstrap_ready', role=role, identity=view['process'])
        return {'identity': view['process']}

    def _ready_view(self, role):
        view = self.inspect_role(role)
        if view['process'] and view['listener_pids'] == [view['process']['pid']] and view['held'] is None:
            # Ready file may precede restored/idle event by a few instructions.
            phases = [row['phase'] for row in self.events(role, view['process']['pid'])]
            if ('restored' if role == 'reservoir' else 'idle') in phases:
                return view
        return None

    def release_hold(self, role, guard):
        self.check_owned()
        self.env.release_hold(self.control(role), guard)
        self.guards.pop(role, None)
        return {'released': True}

    def cleanup_success(self):
        # Success cleanup repeats the reviewed cooperative path; failure cleanup
        # never calls this method and never boots out a live stateful worker.
        cleanup_requests = []
        original_requests = {row.get('request_id') for row in self.events(
            'provider', self.plan['roles']['provider']['identity']['pid']) if row['phase'] == 'checkin_acknowledged'}
        for role in m.ROLES:
            view = self.inspect_role(role)
            expected = view['process']
            m.require(expected is not None and view['held'] is None, 'cleanup needs an exact current worker')
            guard = {'schema': 'model_environment_hold_v1', 'transaction_id': self.token, 'actor': 'owned-fixture-cleanup'}
            self.create_hold(role, guard)
            self.arm_next_spawn(role, guard, expected)
            self.signal_stop(role, expected)
            holder = wait_for(lambda: self.inspect_role(role).get('held'), role + ' cleanup cooperative hold', timeout=90)
            rows = self.events(role, expected['pid'])
            m.require(rows[-1]['phase'] == 'drained', 'cleanup worker did not drain')
            if role == 'provider':
                acknowledgements = [row for row in rows if row['phase'] == 'checkin_acknowledged']
                m.require(len(acknowledgements) == 1 and acknowledgements[0].get('ticks') == 11
                          and acknowledgements[0].get('request_id') not in original_requests,
                          'cleanup check-in is missing or reused an original request')
                cleanup_requests.append(acknowledgements[0]['request_id'])
            self.remove_held(role, holder['identity'], guard)
            self.release_hold(role, guard)
        views = {role: self.inspect_role(role) for role in m.ROLES}
        absent = all(not view['job_loaded'] and view['process'] is None and not view['listener_pids']
                     for view in views.values())
        expected_state = {'fixture_astrid': {'ticks': 22}, 'fixture_minime': {'ticks': 7}}
        m.require(m.read_record(self.root / 'state.json') == expected_state, 'cleanup state does not include both unique check-ins')
        m.require(m.read_record(self.root / 'preserved-state.json') ==
                  {'fixture_astrid': {'ticks': 11}, 'fixture_minime': {'ticks': 7}}, 'original fixture backup changed')
        return {'both_jobs_absent': absent, 'force_used': False, 'stateful_worker_bootout': False,
                'cleanup_request_ids': cleanup_requests, 'final_state': expected_state}

    monotonic = staticmethod(time.monotonic)
    sleep = staticmethod(time.sleep)


def run_fixture(output, reviewed_bindings, *, sudo_debug=False):
    m.require(os.geteuid() != 0, 'run as ordinary user, never sudo Python')
    if sudo_debug:
        m.require(sys.stdin.isatty(), 'interactive Terminal required')
    artifacts = verify_review(reviewed_bindings)
    output = Path(output).absolute()
    EVIDENCE_ROOT.mkdir(parents=True, exist_ok=True)
    m.require(output.parent == EVIDENCE_ROOT.resolve() and not output.exists(), 'output must be new and inside fixture evidence directory')
    token = uuid.uuid4().hex
    root = EVIDENCE_ROOT / ('owned-fixture-' + token)
    root.mkdir(mode=0o700)
    marker = {'schema': OWNER_SCHEMA, 'token': token, 'root': str(root), 'uid': os.getuid()}
    m.write_new(root / 'fixture.owner.json', marker)
    for source in (HOLD_SOURCE, HERE / 'maintenance_fixture_worker.py'):
        with (root / source.name).open('xb') as handle:
            handle.write(source.read_bytes()); handle.flush(); os.fsync(handle.fileno())
    for name, content in (('reservoir-source.marker', 'original fixture marker\n'),
                          ('reservoir-repair.marker', 'repaired fixture marker\n'),
                          ('model-never-opened.sentinel', 'never import or read as a model\n')):
        (root / name).write_text(content)
    result = {'schema': 'reservoir_maintenance_launchd_fixture_v1', 'passed': False,
              'scope': 'owned_fixture_only_no_real_service_or_persistence_acceptance',
              'root': str(root), 'transaction_id': token, 'review_sha256': sha(reviewed_bindings),
              'artifacts': artifacts, 'sudo_debug_requested': sudo_debug,
              'production_qualified': False, 'cleanup': {'both_jobs_absent': False}}
    m.write_new(output, result)
    host = None
    try:
        host = FixtureHost(root, token, Path(sys.executable).resolve(), artifacts, sudo_debug=sudo_debug)
        result['host_binding'] = host.host_binding
        result['python_executables'] = {str(path): sha(path) for path in host.env.interpreter_paths(host.executable)}
        for role in m.ROLES:
            settings = host.plist_settings(role)
            with (root / f'{role}.plist').open('xb') as handle:
                handle.write(plistlib.dumps(settings)); handle.flush(); os.fsync(handle.fileno())
        identities = {}
        for role in ('reservoir', 'provider'):
            identities[role] = host.bootstrap_original(role, {})['identity']
        plan = {'schema': 'reservoir_maintenance_plan_v1', 'transaction_id': token,
                'actor': 'owned-fixture-only', 'pause_generation': 1, 'host_binding': host.host_binding,
                'input_hashes': host.inputs(), 'roles': {role: {'label': m.LABELS[role], 'port': m.PORTS[role],
                'identity': identities[role]} for role in m.ROLES}, 'protected': {}, 'timeout_s': 90}
        host.plan = m.validate_plan(plan)
        journal = m.Journal(root / 'transaction', plan=host.plan)
        result['controller'] = m.Controller(journal, host).run()
        result['cleanup'] = host.cleanup_success()
        result['passed'] = result['cleanup']['both_jobs_absent'] is True
    except BaseException as error:
        result['failure'] = {'type': type(error).__name__, 'message': str(error),
                             'automatic_cleanup': 'none; retain jobs, holds and evidence for reviewed recovery'}
        if host is not None:
            try:
                result['retained_roles'] = {role: host.inspect_role(role) for role in m.ROLES}
            except BaseException as inspection_error:
                result['inspection_error'] = str(inspection_error)
    finally:
        # Only this newly created evidence file is atomically updated. Runtime
        # trees are never deleted, even on success, interruption or timeout.
        temporary = output.with_name(output.name + '.' + token + '.tmp')
        m.write_new(temporary, result)
        os.replace(temporary, output)
        m.sync_dir(output.parent)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-owned-host-fixture', action='store_true', required=True)
    parser.add_argument('--sudo-debug', action='store_true')
    parser.add_argument('--reviewed-bindings', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = run_fixture(args.output, args.reviewed_bindings, sudo_debug=args.sudo_debug)
    print(json.dumps(result, indent=2))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
