"""Separate private plist/argv primitive. Does not execute production pin phase."""
from __future__ import annotations

import os
from pathlib import Path
import plistlib

from owned_fixture_records import (Ledger, digest, directory, raw, read, reference,
                                   require, sync, target, write)

ACTIONS = ('replace_plist', 'bootstrap', 'verify_exit', 'remove_absent_job', 'complete')


def settings(host, model):
    root = host.root
    return {'Label': target(host.uid, host.token, 'pin').split('/')[-1],
            'ProgramArguments': [str(host.executable), str(root / 'owned_pin_sentinel.py'),
              '--root', str(root), '--token', host.token, '--model', str(model),
              '--output', str(root / 'pin/sentinel-exit-intent.json')],
            'WorkingDirectory': str(root), 'RunAtLoad': True, 'KeepAlive': False,
            'StandardOutPath': str(root / 'pin/stdout'), 'StandardErrorPath': str(root / 'pin/stderr')}


def write_bytes(path, data):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    try:
        with os.fdopen(fd, 'wb', closefd=False) as stream:
            stream.write(data); stream.flush(); os.fsync(fd)
    finally: os.close(fd)
    sync(Path(path).parent)


def initialize(host, core):
    require(host.scope == 'owned_fixture', 'pin primitive requires private fixture')
    with core['transaction_execution'].ExecutionSession(host.root, host.plan, host.executor_identity(), 'resume_one'):
        host.check_owned()
        pin = host.root / 'pin'
        binding = {'token': host.token, 'plan_sha256': digest(host.plan),
                   'target': target(host.uid, host.token, 'pin'), 'scope': 'private_pin_primitive_only'}
        if not pin.exists(): ledger = Ledger(host.root, 'pin', binding, create=True)
        else:
            directory(pin)
            if not (pin / 'header.json').exists():
                require(not list(pin.iterdir()), 'unknown partial pin initialization')
                write(pin / 'header.json', {'schema': 'owned_fixture_ledger_v1', 'name': 'pin', 'binding': binding})
            ledger = Ledger(host.root, 'pin', binding)
            require(not ledger.rows, 'pin already attempted; attach one exact phase')
        original = settings(host, pin / 'model-a.sentinel'); candidate = settings(host, pin / 'model-b.sentinel')
        before = list(original['ProgramArguments']); after = list(candidate['ProgramArguments'])
        index = before.index('--model') + 1
        require(before[:index] == after[:index] and before[index+1:] == after[index+1:], 'pin delta differs')
        for name, value in (('original.plist', original), ('candidate.plist', candidate), ('active.plist', original)):
            data = plistlib.dumps(value)
            if (pin / name).exists(): require(raw(pin / name) == data, 'partial pin plist differs')
            else: write_bytes(pin / name, data)
        for name in ('model-a.sentinel', 'model-b.sentinel'):
            data = b'inert path only; never open as model\n'
            if (pin / name).exists(): require(raw(pin / name) == data, 'partial pin marker differs')
            else: write_bytes(pin / name, data)
        inputs = {'binding': binding, 'original': reference(pin / 'original.plist'),
                                  'candidate': reference(pin / 'candidate.plist'),
                                  'sentinel': reference(host.root / 'owned_pin_sentinel.py'),
                                  'record_helper': reference(host.root / 'owned_fixture_records.py'),
                                  'production_pin_schedule_executed': False}
        if (pin / 'inputs.json').exists(): require(read(pin / 'inputs.json') == inputs, 'pin initialization differs')
        else: write(pin / 'inputs.json', inputs)
        host.check_owned()
        return ledger.head


def verify_exit(host, io, *, wait=False):
    """Initial observation may wait within the existing Host deadline, never relaunch."""
    pin = host.root / 'pin'; argv = settings(host, pin / 'model-b.sentinel')['ProgramArguments']
    def observe():
        try: receipt = read(pin / 'sentinel-exit-intent.json')
        except FileNotFoundError:
            observed = io.pin_observe(target(host.uid, host.token, 'pin'), None)
            process = observed.get('process')
            require(process is None or host.env.command_matches(process.get('argv', []), argv),
                    'foreign pin process while awaiting receipt')
            return None
        require(receipt.get('token') == host.token and host.env.command_matches(receipt.get('argv', []), argv)
                and receipt.get('model_argument') == str(pin / 'model-b.sentinel')
                and receipt.get('model_opened') is False and receipt.get('listener_opened') is False
                and receipt.get('numeric_imported') is False and type(receipt.get('pid')) is int,
                'pin argv/scope evidence differs')
        observed = io.pin_observe(target(host.uid, host.token, 'pin'), receipt['pid'])
        process = observed.get('process')
        require(process is None or (process.get('pid') == receipt['pid']
                and host.env.command_matches(process.get('argv', []), argv)), 'foreign pin process')
        if not (observed.get('job_loaded') is True and process is None
                and observed.get('recorded_pid_absent') is True and observed.get('runs') == 1
                and observed.get('last_exit_code') == 0 and not observed.get('listener_pids')): return None
        return {'passed': True, 'receipt': reference(pin / 'sentinel-exit-intent.json'),
                'observation': observed, 'production_pin_schedule_executed': False}
    result = host.wait(observe, 'pin natural exit') if wait else observe()
    require(result is not None, 'pin child natural successful exit is not proven')
    return result


def step(host, core, io, *, expected_head, action, mode, boundary_hook=None):
    require(host.scope == 'owned_fixture' and action in ACTIONS and mode in ('execute', 'reconcile'),
            'explicit private pin action required')
    hook = boundary_hook or (lambda *_: None)
    with core['transaction_execution'].ExecutionSession(host.root, host.plan, host.executor_identity(), 'resume_one'):
        host.check_owned()
        pin = directory(host.root / 'pin'); inputs = read(pin / 'inputs.json')
        ledger = Ledger(host.root, 'pin', inputs['binding']); done = ledger.completed()
        require(ledger.head == expected_head and list(done) == list(ACTIONS[:len(done)])
                and len(done) < len(ACTIONS) and ACTIONS[len(done)] == action, 'stale/wrong pin phase')
        for key in ('original', 'candidate', 'sentinel', 'record_helper'):
            require(reference(inputs[key]['path']) == inputs[key], 'private pin input changed')
        own_target = target(host.uid, host.token, 'pin')
        require(inputs['binding']['target'] == own_target, 'foreign pin label')
        attempted = ledger.attempted(action)
        require(attempted == (mode == 'reconcile'), 'pin attempt cannot be repeated')
        claim = ledger.append('claim' if mode == 'execute' else 'reconcile_claim', action,
                              target=own_target, executor_identity=host.executor_identity())
        active = pin / 'active.plist'; candidate = raw(pin / 'candidate.plist')
        try:
            if action == 'replace_plist':
                if mode == 'execute':
                    require(raw(active) == raw(pin / 'original.plist'), 'private original plist differs')
                    observed = io.pin_observe(own_target, None)
                    require(not observed['job_loaded'] and observed['process'] is None and not observed['listener_pids'],
                            'pin job already exists')
                    temporary = pin / 'active.plist.installing'
                    write_bytes(temporary, candidate)
                    os.replace(temporary, active); sync(pin)
                    artifact = write(pin / 'plist-replaced.json', {'active': reference(active), 'candidate': inputs['candidate']})
                    hook('plist_replaced', artifact)
                require(raw(active) == candidate, 'private applied plist not exact candidate')
                result = {'active': reference(active), 'applied': True}
            elif action == 'bootstrap':
                require(raw(active) == candidate, 'pin bootstrap plist changed')
                if mode == 'execute':
                    observed = io.pin_observe(own_target, None)
                    require(not observed['job_loaded'] and observed['process'] is None and not observed['listener_pids'],
                            'pin target already present')
                    outcome = io.pin_bootstrap(active, own_target)
                    artifact = write(pin / 'bootstrap-return.json', {'target': own_target, 'outcome': outcome})
                    hook('bootstrap_returned', artifact)
                recorded = read(pin / 'bootstrap-return.json')
                require(recorded['target'] == own_target and recorded['outcome']['returncode'] == 0,
                        'pin bootstrap return unproved; never repeat')
                result = {'bootstrapped': True, 'return': reference(pin / 'bootstrap-return.json')}
            elif action == 'verify_exit': result = verify_exit(host, io, wait=True)
            elif action == 'remove_absent_job':
                if mode == 'execute':
                    verify_exit(host, io)
                    outcome = io.pin_remove_absent(own_target, read(pin / 'sentinel-exit-intent.json')['pid'])
                    artifact = write(pin / 'remove-return.json', {'target': own_target, 'outcome': outcome})
                    hook('removed_absent_job', artifact)
                    require(outcome['returncode'] == 0, 'pin unloaded outcome unproved')
                receipt = read(pin / 'sentinel-exit-intent.json')
                observed = io.pin_observe(own_target, receipt['pid'])
                require(not observed['job_loaded'] and observed['process'] is None
                        and observed['recorded_pid_absent'] is True and not observed['listener_pids'],
                        'pin job/process/listener remains')
                result = {'absent': True, 'observation': observed}
            else:
                require(done['remove_absent_job']['absent'] is True, 'pin cleanup not complete')
                receipt = read(pin / 'sentinel-exit-intent.json'); observed = io.pin_observe(own_target, receipt['pid'])
                require(not observed['job_loaded'] and observed['process'] is None
                        and observed['recorded_pid_absent'] is True and not observed['listener_pids'], 'pin no longer absent')
                result = {'passed': True, 'scope': 'private_pin_primitive_only', 'production_pin_schedule_executed': False,
                          'ProductionHost_executed': False, 'production_qualified': False}
            values = {'claim_sha256': digest(claim), 'result': result}
            if mode == 'reconcile': values.update(resolves_claims=ledger.outstanding(action),
                proof={'passed': True, 'effect_only': True, 'producer_replayed': False, 'result': result})
            ledger.append('result' if mode == 'execute' else 'reconciled', action, **values)
            host.check_owned()
            return {'head_sha256': ledger.head, 'result': result, 'automatic_next_action': False}
        except BaseException as error:
            ledger.append('failed', action, claim_sha256=digest(claim), error_type=type(error).__name__,
                          error=str(error)[:1000], retry_permitted=False)
            raise
