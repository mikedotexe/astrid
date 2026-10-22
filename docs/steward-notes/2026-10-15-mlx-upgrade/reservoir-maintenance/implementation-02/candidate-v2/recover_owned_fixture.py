#!/usr/bin/env python3
"""Recover only admin-01's retained fake workers; no production adapter.

The failed fixture receipt and original source remain immutable. Every recovery
attempt has a new receipt and appends operations after the original 27. A
successful recovery proves cleanup only, not a fresh successful fixture run.
"""
from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys

import qualify_reservoir_maintenance as q

m = q.m
HERE = Path(__file__).resolve().parent
TARGET = HERE / 'recovery-target.json'
TOKEN = '3b6d224c3c42468c8033890d2c535662'
ROOT = HERE.parent / 'evidence' / ('owned-fixture-' + TOKEN)
GUARD = {'schema': 'model_environment_hold_v1', 'transaction_id': TOKEN,
         'actor': 'owned-fixture-cleanup'}
PYTHON = Path('/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14')


def required_artifacts():
    return [Path(__file__).resolve(), TARGET, HERE / 'test_recover_owned_fixture.py',
            *q.required_artifacts()]


def verify_review(path):
    review = m.read_record(path)
    expected = {str(p): q.sha(p) for p in required_artifacts()}
    m.require(review.get('schema') == 'owned_fixture_cleanup_review_v1'
              and review.get('approved') is True and review.get('artifacts') == expected,
              'recovery helpers need an exact accepted review')
    ref = review.get('cpu_receipt', {})
    m.require(isinstance(ref.get('path'), str) and q.sha(ref['path']) == ref.get('sha256'),
              'reviewed CPU receipt changed')
    cpu = m.read_record(Path(ref['path']))
    m.require(cpu.get('passed') is True and cpu.get('artifacts') == expected,
              'CPU receipt does not bind recovery helpers')
    return expected


def verify_retained(target):
    m.require(target.get('schema') == 'owned_fixture_cleanup_target_v1'
              and target.get('root') == str(ROOT) and target.get('transaction_id') == TOKEN
              and target.get('original_operations') == 27, 'wrong recovery target')
    for path, digest in target['immutable_files'].items():
        m.require(q.sha(path) == digest, 'retained evidence changed: ' + path)
    prefix = target['reservoir_event_prefix']
    data = (ROOT / 'reservoir.events.jsonl').read_bytes()
    m.require(len(data) >= prefix['bytes'] and
              hashlib.sha256(data[:prefix['bytes']]).hexdigest() == prefix['sha256'],
              'original reservoir event prefix changed')
    original = m.read_record(Path(target['failed_receipt']))
    journal = m.Journal(ROOT / 'transaction')
    m.require(len(journal.events) == 38 and journal.events[-1]['type'] == 'completed'
              and journal.events[-1]['receipt'] == original.get('controller')
              and original['controller']['status'] == 'verified'
              and original.get('passed') is False and original.get('production_qualified') is False,
              'original controller did not complete exactly as retained')
    request_id = target['second_request']
    ack = m.read_record(ROOT / ('ack.' + request_id + '.json'))
    m.require(ack == {'ok': True, 'request_id': request_id, 'ticks': 11, 'token': TOKEN},
              'second check-in acknowledgement mismatch')
    return original, journal


def operation_records():
    result = []
    for index, path in enumerate(sorted(ROOT.glob('host-operation-*.json')), 1):
        m.require(path.name == f'host-operation-{index:05d}.json', 'operation sequence gap')
        result.append(m.read_record(path))
    m.require(len(result) >= 27, 'original operation prefix missing')
    return result


class RecoveryHost(q.FixtureHost):
    """Same allowlisted adapter, with a single reservoir debug privilege edge."""
    def __init__(self, target, artifacts, original, journal, *, interactive_debug):
        super().__init__(ROOT, TOKEN, PYTHON, artifacts, sudo_debug=False)
        self.plan = journal.plan
        m.require(self.host_binding == original['host_binding'], 'host or boot changed')
        m.require({str(p): q.sha(p) for p in self.env.interpreter_paths(PYTHON)} ==
                  original['python_executables'], 'interpreter identity changed')
        self.interactive_debug = interactive_debug
        self.target_record = target
        self.operations = len(operation_records())
        self.guards['provider'] = GUARD
        if (self.control('reservoir') / 'hold.json').exists():
            self.env.assert_hold(self.control('reservoir'), GUARD)
            self.guards['reservoir'] = GUARD
        self.ports['reservoir'] = 52970
        # Preserve old sentinel checks even after that listener closes.
        for role in m.ROLES:
            settings = plistlib.loads((ROOT / f'{role}.plist').read_bytes())
            m.require(settings == self.plist_settings(role), 'retained plist changed')
        for row in operation_records()[27:]:
            if row.get('operation') == 'signal_intent' and row.get('role') == 'reservoir':
                ident = row.get('identity')
                m.require(ident == target['reservoir_worker'], 'foreign recovery signal intent')
                self.signalled.add(('reservoir', ident['pid'], ident['started_at']))

    def arm_next_spawn(self, role, guard, expected):
        m.require(role == 'reservoir' and expected == self.target_record['reservoir_worker'],
                  'recovery may arm only the exact retained reservoir worker')
        self.check_owned()
        self.env.assert_hold(self.control(role), guard)
        view = self.inspect_role(role)
        m.require(view['process'] == expected and view['held'] is None, 'worker changed before recovery arm')
        target = self.target(role)
        q.validate_target(target, self.uid, TOKEN, role)
        argv = ['/usr/bin/sudo']
        if self.interactive_debug:
            m.require(sys.stdin.isatty(), 'interactive debug requires Terminal stdin')
        else:
            argv.append('-n')
        argv += ['--', '/bin/launchctl', 'debug', target, '--program', str(PYTHON),
                 '--', *self.hold_argv(role)[1:]]
        self.operation('debug_intent', role=role, command=argv, recovery=True)
        # Interactive stdin/stderr stay attached to the Terminal. Only -n
        # output is captured: it cannot prompt or receive a password. A generic
        # nonzero exit does not prove launchctl never installed the override.
        options = {'check': False, 'timeout': 180}
        if not self.interactive_debug:
            options.update(capture_output=True, text=True)
        result = subprocess.run(argv, **options)
        stdout, stderr = (result.stdout or ''), (result.stderr or '')
        auth_refused = (not self.interactive_debug and result.returncode != 0
                        and not stdout.strip() and stderr.strip() == 'sudo: a password is required')
        self.operation('debug_result', role=role, returncode=result.returncode, recovery=True,
                       authorization_refused_before_command=auth_refused,
                       stdout=stdout[:4096], stderr=stderr[:4096])
        m.require(result.returncode == 0,
                  ('sudo refused before command; guard retained and worker not signalled' if auth_refused else
                   'debug outcome uncertain; guard retained for explicit reconciliation'))
        self.armed.add((role, expected['pid'], expected['started_at']))
        return {'armed': True}


def verify_second_checkin(host, target):
    rows = host.events('provider', 76512)
    m.require([r['phase'] for r in rows] == ['started', 'idle', 'generation_admitted',
              'stop_received', 'checkin_sent', 'checkin_acknowledged', 'drained'],
              'retained provider did not complete its exact drain')
    m.require(all(row.get('request_id') == target['second_request'] for row in rows[2:])
              and rows[-1]['monotonic'] - rows[3]['monotonic'] >= 6, 'provider drain binding differs')
    applied = [r for r in host.events('reservoir', 76424) if r['phase'] == 'checkin_applied']
    m.require(len(applied) == 1 and applied[0]['request_id'] == target['second_request']
              and applied[0]['state'] == target['expected_final_state'], 'second check-in was not applied')


def remove_provider_holder(host, target):
    view = host.inspect_role('provider')
    expected = dict(target['provider_holder'], argv=host.hold_argv('provider'))
    if view['job_loaded']:
        held = view.get('held')
        m.require(held is not None and held['guard'] == GUARD
                  and held['identity']['pid'] == expected['pid']
                  and held['identity']['started_at'] == expected['started_at']
                  and host.env.command_matches(held['identity']['argv'], expected['argv']),
                  'recovery provider is not the exact retained holder')
        host.remove_held('provider', held['identity'], GUARD)
    else:
        # Continuation may observe the prior reviewed holder removal, never a
        # new arbitrary absent job. A bootout intent is sufficient after crash
        # only with independently proven job/PID/listener absence below.
        removals = [r for r in operation_records()[27:] if r.get('operation') in
                    ('held_bootout_intent', 'held_job_absent') and r.get('role') == 'provider']
        m.require(removals and all(r['identity']['pid'] == 77047 and
                  r['identity']['started_at'] == target['provider_holder']['started_at']
                  for r in removals), 'provider absence lacks this recovery removal evidence')
    m.require(host.env.process_start(77047) is None and not host.listener_ports(77047),
              'old provider holder still survives')
    if (host.control('provider') / 'hold.json').exists():
        host.release_hold('provider', GUARD)


def verify_final_snapshot(host, target):
    rows = host.events('reservoir', 76424)
    tail = [r for r in rows if r['phase'] in ('stop_received', 'final_snapshot_started',
                                             'handle_saved', 'final_snapshot_complete', 'drained')]
    m.require([r['phase'] for r in tail] == ['stop_received', 'final_snapshot_started',
              'handle_saved', 'handle_saved', 'final_snapshot_complete', 'drained'],
              'recovery reservoir final snapshot is incomplete')
    m.require(tail[-1]['monotonic'] - tail[0]['monotonic'] >= 6
              and tail[1]['handles'] == ['fixture_astrid', 'fixture_minime']
              and {r['name']: r['value'] for r in tail if r['phase'] == 'handle_saved'} ==
              target['expected_final_state'] and tail[-2]['state'] == target['expected_final_state'],
              'recovery final handle state differs')
    m.require(m.read_record(ROOT / 'state.json') == target['expected_final_state'],
              'final state file does not include both unique check-ins')


def debug_state(records, host):
    """Never replay an uncertain or already successful one-shot override."""
    state = 'never'
    tail = ['--', '/bin/launchctl', 'debug', host.target('reservoir'), '--program',
            str(PYTHON), '--', *host.hold_argv('reservoir')[1:]]
    for row in records:
        if row.get('role') != 'reservoir':
            continue
        if row.get('operation') == 'debug_intent':
            m.require(state in ('never', 'failed'), 'debug override was repeated after uncertain or successful arm')
            m.require(row.get('recovery') is True and row.get('command') in
                      (['/usr/bin/sudo', *tail], ['/usr/bin/sudo', '-n', *tail]),
                      'recovery debug intent is outside exact owned target')
            previous_command = row['command']
            state = 'uncertain'
        elif row.get('operation') == 'debug_result':
            m.require(state == 'uncertain' and row.get('recovery') is True
                      and type(row.get('returncode')) is int, 'unmatched recovery debug result')
            if row['returncode'] == 0:
                state = 'armed'
            else:
                known_refusal = (row.get('authorization_refused_before_command') is True
                                 and not row.get('stdout', '').strip()
                                 and row.get('stderr', '').strip() == 'sudo: a password is required'
                                 and previous_command[:3] == ['/usr/bin/sudo', '-n', '--'])
                state = 'failed' if known_refusal else 'uncertain'
    return state


def finish_reservoir(host, target):
    records = operation_records()[27:]
    expected = target['reservoir_worker']
    intents = [r for r in records if r.get('operation') == 'signal_intent'
               and r.get('role') == 'reservoir']
    m.require(len(intents) <= 1 and all(r.get('identity') == expected for r in intents),
              'multiple or foreign recovery signal intents')
    view = host.inspect_role('reservoir')
    if view['job_loaded'] and view.get('held') is None:
        m.require(view['process'] == expected or (intents and view['process'] is None),
                  'retained reservoir worker identity changed')
        if not intents:
            if 'reservoir' not in host.guards:
                host.create_hold('reservoir', GUARD)
            prior_debug = debug_state(records, host)
            m.require(prior_debug != 'uncertain', 'debug outcome is uncertain; retain hold for explicit reconciliation')
            if prior_debug == 'armed':
                host.env.assert_hold(host.control('reservoir'), GUARD)
                host.armed.add(('reservoir', expected['pid'], expected['started_at']))
            else:
                host.arm_next_spawn('reservoir', GUARD, expected)
            host.signal_stop('reservoir', expected)
        # An interrupted signal intent is never repeated. It may have been
        # delivered; wait for evidence, or retain the worker without escalation.
        def held_view():
            current = host.inspect_role('reservoir')
            m.require(current['job_loaded'], 'reservoir job unexpectedly unloaded while draining')
            m.require(current.get('held') is not None or current.get('process') in (None, expected),
                      'foreign reservoir replacement while draining')
            return current if current.get('held') else None
        view = q.wait_for(held_view, 'retained reservoir cooperative hold', timeout=90)
    else:
        m.require(intents, 'reservoir holder or absence lacks the exact cooperative signal intent')
    verify_final_snapshot(host, target)
    m.require(host.env.process_start(expected['pid']) is None and not host.listener_ports(expected['pid']),
              'old reservoir worker survives beside its replacement')
    if view['job_loaded']:
        held = view.get('held')
        m.require(held is not None and held['guard'] == GUARD, 'reservoir did not enter owned hold')
        host.remove_held('reservoir', held['identity'], GUARD)
    else:
        removals = [r for r in records if r.get('operation') in ('held_bootout_intent', 'held_job_absent')
                    and r.get('role') == 'reservoir']
        m.require(removals, 'reservoir absence lacks this recovery removal evidence')
        for row in removals:
            identity = row.get('identity', {})
            m.require(host.env.command_matches(identity.get('argv'), host.hold_argv('reservoir'))
                      and type(identity.get('pid')) is int and identity['pid'] != expected['pid']
                      and isinstance(identity.get('started_at'), str)
                      and host.env.process_start(identity['pid']) is None
                      and not host.listener_ports(identity['pid']), 'removed reservoir holder survives or differs')
    if (host.control('reservoir') / 'hold.json').exists():
        host.release_hold('reservoir', GUARD)


def run(output, review, *, interactive_debug=False):
    m.require(os.geteuid() != 0 and os.getuid() == 501, 'run as original ordinary user, never sudo Python')
    m.require(Path(sys.executable).resolve() == PYTHON, 'use exact reviewed Python 3.14')
    artifacts = verify_review(review)
    target = m.read_record(TARGET)
    original, journal = verify_retained(target)
    output = Path(output).absolute()
    m.require(output.parent == q.EVIDENCE_ROOT.resolve() and not output.exists(), 'new recovery receipt required')
    host = None
    result = {'schema': 'owned_fixture_cleanup_result_v1', 'passed': False,
              'production_qualified': False, 'original_fixture_passed': False,
              'original_receipt': {'path': target['failed_receipt'], 'sha256': q.sha(target['failed_receipt'])},
              'root': str(ROOT), 'transaction_id': TOKEN, 'artifacts': artifacts,
              'review_sha256': q.sha(review), 'interactive_debug': interactive_debug}
    lock_fd = os.open(ROOT / 'recovery.lock', os.O_CREAT | os.O_RDWR | os.O_NOFOLLOW, 0o600)
    with os.fdopen(lock_fd, 'w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        try:
            # Same worker bytes at the v2 path; original seven-file identity is
            # independently verified by the retained target manifest above.
            host = RecoveryHost(target, artifacts, original, journal, interactive_debug=interactive_debug)
            result['operations_before'] = host.operations
            verify_second_checkin(host, target)
            host.operation('recovery_started', recovery_receipt=str(output), original_receipt_sha256=q.sha(target['failed_receipt']))
            remove_provider_holder(host, target)
            finish_reservoir(host, target)
            views = {role: host.inspect_role(role) for role in m.ROLES}
            m.require(all(not v['job_loaded'] and v['process'] is None and not v['listener_pids']
                          for v in views.values()), 'owned jobs or listeners remain')
            m.require(host.env.process_start(76424) is None and not host.listener_ports(76424),
                      'original reservoir worker still survives')
            verify_retained(target)
            result.update(passed=True, cleanup={'both_jobs_absent': True, 'force_used': False,
                          'stateful_worker_bootout': False, 'final_state': target['expected_final_state']})
            host.operation('recovery_completed', recovery_receipt=str(output), final_state=target['expected_final_state'])
        except BaseException as error:
            result['failure'] = {'type': type(error).__name__, 'message': str(error),
                                 'automatic_cleanup': 'none; retain exact owned jobs and guards'}
            result['interactive_continuation'] = [str(PYTHON), '-B', str(Path(__file__).resolve()),
                '--reviewed-bindings', str(review), '--output', str(output.with_name(output.stem + '-continued.json')),
                '--interactive-debug']
            if host is not None:
                try:
                    result['retained_roles'] = {role: host.inspect_role(role) for role in m.ROLES}
                except Exception as inspection_error:
                    result['inspection_error'] = str(inspection_error)
        finally:
            if host is not None:
                result['operations_after'] = host.operations
            m.write_new(output, result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--reviewed-bindings', type=Path, required=True)
    parser.add_argument('--interactive-debug', action='store_true')
    args = parser.parse_args()
    result = run(args.output, args.reviewed_bindings, interactive_debug=args.interactive_debug)
    print(json.dumps({'passed': result['passed'], 'receipt': str(args.output)}))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
