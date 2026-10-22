#!/usr/bin/env python3
"""Finite owned stdlib worker integration; no launchd, sudo or real services.

Run only after source review. Failed/interrupted runs retain all artifacts and
unresolved children. The exclusive active marker blocks another run until the
owner has reviewed recovery. No forced termination or failure cleanup exists.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import subprocess
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE.parent / 'evidence'
EXACT_PYTHON = Path('/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14')
WORKER = HERE / 'maintenance_fixture_worker.py'
HOLD_HELPER = Path('/Users/v/other/astrid/scripts/model_environment.py')


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def now():
    return datetime.now(timezone.utc).isoformat()


def write_new(path, value):
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(descriptor, 'w') as handle:
        json.dump(value, handle, indent=2); handle.write('\n')
        handle.flush(); os.fsync(handle.fileno())
    directory = os.open(path.parent, os.O_RDONLY)
    try:
        os.fsync(directory)
    finally:
        os.close(directory)


class Run:
    def __init__(self, root, token, helper, worker_module, guard, output):
        self.root, self.token, self.helper, self.worker = root, token, helper, worker_module
        self.guard, self.output = guard, output
        self.children, self.cycles, self.requests = [], [], set()
        self.deadline = time.monotonic() + 90
        self.started_at = now()
        self.hashes = {str(path): digest(path) for path in (Path(__file__), WORKER, HOLD_HELPER)}
        self.hashes[str(root / 'maintenance_fixture_worker.py')] = digest(root / 'maintenance_fixture_worker.py')
        require(self.hashes[str(root / 'maintenance_fixture_worker.py')] == self.hashes[str(WORKER)],
                'private worker copy differs from reviewed source')
        self.hashes.update({str(path): digest(path) for path in helper.interpreter_paths(EXACT_PYTHON)})

    def save_guard(self):
        self.worker.atomic(self.guard, {'schema': 'owned_fixture_worker_active_v1', 'token': self.token,
            'root': str(self.root), 'runner_pid': os.getpid(), 'children': [item['record'] for item in self.children]})

    def current_identity(self, child):
        process, record = child['process'], child['record']
        if process.poll() is not None:
            return None
        start = self.helper.process_start(process.pid)
        result = subprocess.run(['/bin/ps', '-ww', '-p', str(process.pid), '-o', 'command='],
                                capture_output=True, text=True, check=True, timeout=5)
        actual = shlex.split(result.stdout.strip())
        require(start is not None and self.helper.command_matches(actual, record['argv']), 'owned child command mismatch')
        if record.get('started_at'):
            require(start == record['started_at'], 'owned child start changed')
        require(self.helper.process_start(process.pid) == start and process.poll() is None,
                'child changed during identity check')
        return {'pid': process.pid, 'started_at': start, 'argv': actual}

    def events(self, child):
        record = child['record']
        path = self.root / (record['role'] + '.events.jsonl')
        if not path.exists():
            return []
        data = path.read_bytes()
        data = data[:data.rfind(b'\n') + 1]
        rows = [json.loads(line) for line in data.splitlines()]
        require(all(row['token'] == self.token and row['role'] == record['role'] for row in rows), 'foreign worker event')
        return [row for row in rows if row['pid'] == record['pid']]

    def wait(self, predicate, label, timeout=20):
        deadline = min(self.deadline, time.monotonic() + timeout)
        while time.monotonic() < deadline:
            result = predicate()
            if result:
                return result
            time.sleep(.05)
        raise RuntimeError('bounded wait expired without escalation: ' + label)

    def start(self, role, cycle):
        require(all(item['process'].poll() is not None or item['record']['cycle'] == cycle for item in self.children),
                'unresolved earlier child blocks a new cycle')
        require(time.monotonic() < self.deadline, 'global deadline reached before spawn')
        for path, expected in self.hashes.items():
            require(digest(path) == expected, 'reviewed worker/helper/interpreter changed')
        argv = [str(EXACT_PYTHON), '-I', '-B', str(self.root / 'maintenance_fixture_worker.py'),
                '--root', str(self.root), '--token', self.token, '--role', role, '--drain-seconds', '6.0']
        record = {'role': role, 'cycle': cycle, 'argv': argv, 'spawn_intent_at': now(),
                  'signal_sent': False, 'force_used': False}
        write_new(self.root / f'{cycle}-{role}-spawn-intent.json', record)
        with (self.root / f'{cycle}-{role}.stdout').open('xb') as stdout, \
             (self.root / f'{cycle}-{role}.stderr').open('xb') as stderr:
            process = subprocess.Popen(argv, cwd=self.root, stdin=subprocess.DEVNULL,
                                       stdout=stdout, stderr=stderr, start_new_session=True)
        record.update(pid=process.pid, spawned_at=now())
        child = {'process': process, 'record': record}
        self.children.append(child)
        self.save_guard()
        identity = self.wait(lambda: self.current_identity(child), role + ' identity', timeout=5)
        record.update(started_at=identity['started_at'], observed_argv=identity['argv'])
        self.save_guard()

        def ready():
            require(process.poll() is None, role + ' exited before readiness')
            path = self.root / (role + '.ready.json')
            if not path.exists():
                return None
            value = json.loads(path.read_text())
            require(value.get('pid') == process.pid and value.get('token') == self.token
                    and value.get('role') == role and value.get('protocol') == 'fixture_sentinel_only', 'ready identity mismatch')
            require(type(value.get('port')) is int and 49152 <= value['port'] <= 65535, 'unexpected fixture ephemeral port')
            phase = 'restored' if role == 'reservoir' else 'idle'
            if not any(row['phase'] == phase for row in self.events(child)):
                return None
            self.current_identity(child)
            return value

        record['ready'] = self.wait(ready, role + ' ready/restored', timeout=10)
        record['ready_at'] = now()
        self.save_guard()
        return child

    def stop_once(self, child):
        record, process = child['record'], child['process']
        require(not record['signal_sent'], 'repeat signal refused')
        identity = self.current_identity(child)
        require(identity is not None, 'child exited before requested stop')
        record.update(signal_intent_at=now(), signal_identity=identity)
        self.save_guard()
        record.update(signal_sent=True, signal_sent_at=now(), signal_monotonic=time.monotonic())
        os.kill(process.pid, signal.SIGTERM)
        self.save_guard()

        def exited():
            code = process.poll()
            if code is None:
                return None
            record.update(exit_code=code, exited_at=now(), exit_monotonic=time.monotonic())
            require(code == 0, 'owned worker returned failure')
            return True

        self.wait(exited, record['role'] + ' natural exit', timeout=20)
        require(not (self.root / (record['role'] + '.ready.json')).exists(), 'worker left stale ready marker')
        self.save_guard()

    def cycle(self, number):
        reservoir = self.start('reservoir', number)
        restored = [row['state'] for row in self.events(reservoir) if row['phase'] == 'restored']
        expected_before = {'fixture_astrid': {'ticks': (number - 1) * 11}, 'fixture_minime': {'ticks': 7}}
        require(restored == [expected_before], 'initial/restarted fake state differs')
        provider = self.start('provider', number)
        require(reservoir['process'].poll() is None, 'reservoir unavailable before provider drain')
        self.stop_once(provider)
        require(reservoir['process'].poll() is None, 'reservoir did not remain alive for check-in')
        provider_rows = self.events(provider)
        require([row['phase'] for row in provider_rows] == ['started', 'idle', 'generation_admitted',
                'stop_received', 'checkin_sent', 'checkin_acknowledged', 'drained'], 'provider lifecycle differs')
        request = provider_rows[2]['request_id']
        require(request not in self.requests and all(row.get('request_id') == request for row in provider_rows[2:]),
                'missing or repeated request identity')
        self.requests.add(request)
        require(provider_rows[5]['ticks'] == 11 and provider_rows[-1]['monotonic'] - provider_rows[3]['monotonic'] >= 6,
                'provider did not finish a six-second drain and eleven-tick acknowledgement')
        applied = [row for row in self.events(reservoir) if row['phase'] == 'checkin_applied' and row.get('request_id') == request]
        require(len(applied) == 1, 'reservoir acknowledgement was not applied exactly once')
        self.stop_once(reservoir)
        rows = self.events(reservoir)
        final = [row for row in rows if row['phase'] == 'final_snapshot_complete']
        stops = [row for row in rows if row['phase'] == 'stop_received']
        expected_after = {'fixture_astrid': {'ticks': number * 11}, 'fixture_minime': {'ticks': 7}}
        require(len(final) == len(stops) == 1 and final[0]['monotonic'] - stops[0]['monotonic'] >= 6,
                'reservoir did not finish a six-second final snapshot')
        saved = [row for row in rows if row['phase'] == 'handle_saved']
        require(len(saved) == 2 and {row['name']: row['value'] for row in saved} == expected_after,
                'final per-handle evidence is incomplete')
        require(final[0]['state'] == expected_after and json.loads((self.root / 'state.json').read_text()) == expected_after,
                'final fake saved state differs')
        self.cycles.append({'cycle': number, 'request_id': request, 'state_before': expected_before,
                           'state_after': expected_after, 'provider_events': provider_rows, 'reservoir_events': rows,
                           'state_sha256': digest(self.root / 'state.json')})
        write_new(self.root / f'cycle-{number}-receipt.json', self.cycles[-1])

    def execute(self):
        result = {'schema': 'owned_fixture_workers_integration_v1', 'passed': False,
                  'scope': 'owned_stdlib_worker_IPC_and_signals_only', 'launchd_qualified': False,
                  'real_persistence_qualified': False, 'production_qualified': False,
                  'started_at': self.started_at, 'root': str(self.root), 'token': self.token,
                  'artifacts': self.hashes, 'force_used': False, 'launchd_or_sudo_used': False}
        try:
            self.cycle(1)
            self.cycle(2)
            require(all(child['process'].poll() == 0 for child in self.children), 'unresolved worker at completion')
            result['passed'] = True
        except BaseException as error:
            result['failure'] = {'type': type(error).__name__, 'message': str(error),
                                 'recovery': 'No automatic cleanup. Retain guard, children and evidence for owner review.'}
        finally:
            result.update(completed_at=now(), children=[item['record'] for item in self.children], cycles=self.cycles,
                          unresolved_children=[item['record'] for item in self.children if item['process'].poll() is None])
            write_new(self.output, result)
            self.worker.atomic(self.root / 'final-result.json', result)
            if result['passed']:
                current = json.loads(self.guard.read_text())
                require(current.get('token') == self.token and current.get('root') == str(self.root), 'active guard ownership changed')
                write_new(self.root / 'completed-active-guard.json', current)
                self.guard.unlink()
                self.helper.sync_directory(self.guard.parent)
        return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-owned-workers', action='store_true', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    require(os.geteuid() != 0 and Path(sys.executable).resolve() == EXACT_PYTHON.resolve(), 'ordinary user and exact Python 3.14 required')
    EVIDENCE.mkdir(exist_ok=True)
    output = args.output.absolute()
    require(output.parent == EVIDENCE.resolve() and not output.exists(), 'new private evidence output required')
    token = uuid.uuid4().hex
    root = EVIDENCE / ('owned-workers-' + token)
    guard = EVIDENCE / '.owned-worker-integration.active.json'
    write_new(guard, {'schema': 'owned_fixture_worker_active_v1', 'token': token, 'root': str(root), 'children': []})
    root.mkdir(mode=0o700)
    worker = load('frozen_owned_fixture_worker', WORKER)
    helper = load('frozen_owned_hold_helper', HOLD_HELPER)
    worker.atomic(root / 'fixture.owner.json', {'schema': worker.SCHEMA, 'token': token, 'root': str(root), 'uid': os.getuid()})
    with (root / 'maintenance_fixture_worker.py').open('xb') as handle:
        handle.write(WORKER.read_bytes()); handle.flush(); os.fsync(handle.fileno())
    run = Run(root, token, helper, worker, guard, output)
    result = run.execute()
    print(json.dumps({'passed': result['passed'], 'output': str(output), 'root': str(root),
                      'unresolved_children': result['unresolved_children']}, indent=2))
    return 0 if result['passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
