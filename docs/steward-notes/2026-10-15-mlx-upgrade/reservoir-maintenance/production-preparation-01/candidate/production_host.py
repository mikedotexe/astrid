"""Blocked, fixed-two-label production Host candidate; never a service manager.

The copied qualified controller also rejects this production scope. Enabling
either boundary is a separate source change requiring root review, a real
restore matrix and exact host qualification. No execution CLI is provided.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import plistlib
import re
import signal
import subprocess
import sys
import time

import production_preflight as p

m = p.m
PRODUCTION_EXECUTION_ENABLED = False
HOLDER_PYTHON = Path('/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14')


def stamp(path):
    value = Path(path).stat()
    return (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns, value.st_mode, value.st_uid)


def wait_for(predicate, label, timeout):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        value = predicate()
        if value:
            return value
        time.sleep(.1)
    raise m.Refusal(label + ' timed out; no escalation; retain holds and evidence')


def startup_log(data, expected, config):
    text = data.decode()
    m.require(not data or data.endswith(b'\n'), 'incomplete startup log')
    m.require(not re.search(r'restore.*(?:failed|error)|thermostat.*(?:failed|warning)|quarantin|Traceback', text, re.I),
              'restore or thermostat warning/error in startup')
    found = {}
    for line in text.splitlines():
        match = re.search(r"restored '([^']+)' \(entity=(.*?), backend=(.*?), ticks=(\d+), mode=(.*?)\)", line)
        if match:
            name = match[1]
            m.require(name not in found, 'duplicate restored handle')
            found[name] = {'entity': match[2], 'backend': match[3], 'tick_count': int(match[4]), 'mode': match[5]}
    m.require(found == expected, 'complete per-handle startup restoration differs from real restore witness')
    listeners = re.findall(r'listening on ([^\n]+)', text)
    line = (f"127.0.0.1:7881 (input_dim={config['input_dim']}, n_nodes={config['n_nodes']}, "
            f"backend={config['backend_label']})")
    m.require(listeners == [line], 'startup config/listener marker differs')
    return {'restored': found, 'listening': line, 'warnings': []}


def verify_final_files(entries, handles, stopped_ns):
    for name in handles:
        m.require('/' not in name and '\\' not in name and name not in ('.', '..'), 'unsafe final handle name')
        for filename in (name + '.npz', name + '_thermostats.json'):
            item = entries.get(filename, {})
            m.require(item.get('kind') == 'file' and item.get('mtime_ns', -1) >= stopped_ns,
                      'final handle snapshot/sidecar missing or predates cooperative stop: ' + filename)


class ProductionHost:
    scope = 'production'

    def __init__(self, preparation, *, io=None, restore_runner=None):
        self.spec = preparation
        self.io = io or p.ReadOnlySystem()
        self.plan = p.validate_preparation(preparation)
        self.root = Path(preparation['transaction_root'])
        self.env = p.load_hold()
        self.restore_runner = restore_runner
        self.guards, self.armed, self.signalled = {}, set(), set()
        self.anchors, self.receipts, self.startups = {}, {}, {}
        self.repaired = False
        self.provider_pinned = False
        self.signal_times = {}
        self.counter = 0
        self.bound_stats = {path: stamp(path) for path in self.spec['immutable_files']}

    def control(self, role):
        m.require(role in m.ROLES, 'unknown fixed role')
        return self.root / ('hold-' + role)

    def target(self, role):
        m.require(role in m.ROLES, 'unknown fixed role')
        return f"gui/{self.plan['host_binding']['uid']}/{m.LABELS[role]}"

    def hold_argv(self, role):
        return [str(HOLDER_PYTHON), str(self.root / 'model_environment.py'), 'launch',
                '--control-root', str(self.control(role)), '--hold-only', self.plan['transaction_id']]

    def _input_hashes(self):
        # Full package/model content was hashed in preparation. During drain,
        # immutable inode/size/mtime/mode/owner guards avoid repeatedly hashing
        # model weights; canonical source/plists still receive full hashes.
        for path, expected in self.bound_stats.items():
            m.require(stamp(path) == expected, 'immutable input metadata changed: ' + path)
        inputs = dict(self.plan['input_hashes'])
        for role in m.ROLES:
            inputs[role + '_source'] = p.sha(p.SOURCES[role])
            inputs[role + '_settings'] = p.sha(p.PLISTS[role])
        return inputs

    def _ownership(self):
        for key in ('fixture', 'runtime_alignment', 'model_resolution', 'original_provider_manifest'):
            p.reference(self.spec[key])
        current = self.io.stewardship()
        m.require(self.io.host() == self.plan['host_binding'] and current == {'paused': True,
            'pause_generation': self.plan['pause_generation'], 'pause_actor': self.plan['actor'], 'lease': None},
            'host or steward ownership changed')
        for name, identity in self.plan['protected'].items():
            m.require(self.io.process(identity['pid']) == identity, 'protected process changed: ' + name)
        expected = dict(self.plan['input_hashes'])
        if self.repaired:
            expected['reservoir_source'] = expected['reservoir_candidate']
        if self.provider_pinned:
            expected['provider_settings'] = expected['provider_restart_settings']
        m.require(self._input_hashes() == expected, 'bound source/configuration changed')

    def inspect(self):
        self._ownership()
        return {'host_binding': self.plan['host_binding'], **self.io.stewardship(),
                'input_hashes': self._input_hashes(), 'protected': self.plan['protected'],
                'roles': {role: self.inspect_role(role) for role in m.ROLES},
                'originals_preserved': True, 'rollback_software_ready': True,
                'fixture_mode_authorized': False, 'execution_enabled': False,
                'runtime_alignment': self.spec['runtime_alignment']}

    def inspect_role(self, role):
        view = self.io.role(role)
        m.require(view['target'] == self.target(role), 'wrong production target')
        process = view.get('process')
        if process is None:
            return view
        normal_argv = (self.spec['provider_restart_argv'] if role == 'provider' and self.provider_pinned else
                       self.plan['roles'][role]['identity']['argv'])
        if self.env.command_matches(process['argv'], normal_argv):
            return view
        m.require(self.env.command_matches(process['argv'], self.hold_argv(role)), 'unexpected replacement command')
        m.require(not view['listener_pids'], 'holder has a production listener')
        # Holder must have no other TCP listener either, not merely the role port.
        listeners = self.io.command('/usr/sbin/lsof', '-nP', '-a', '-p', str(process['pid']), '-iTCP', '-sTCP:LISTEN', '-Fp')
        m.require(listeners.returncode == 1 and not listeners.stdout.strip() and not listeners.stderr.strip(),
                  'holder is not listener-free')
        receipt_path = self.control(role) / 'held' / (str(process['pid']) + '.json')
        if not receipt_path.exists():
            return dict(view, process=None)
        receipt = m.read_record(receipt_path)
        guard = self.guards.get(role)
        m.require(guard and receipt.get('hold') == guard and receipt.get('started_at') == process['started_at']
                  and receipt.get('pid') == process['pid'] and receipt.get('hold_only') is True
                  and receipt.get('model_imported') is False and receipt.get('listener_opened') is False
                  and receipt.get('launcher_sha256') == p.sha(self.root / 'model_environment.py'),
                  'holder receipt/identity mismatch')
        self.env.assert_hold(self.control(role), guard)
        return dict(view, held={'guard': guard, 'identity': process, 'hold_only': True,
                               'stdlib_only': True, 'state_touched': False})

    def prepare_capture(self):
        """Explicit read-only service queries plus fresh private evidence writes."""
        p.check_only(self.spec, self.io)
        m.require(not self.root.exists() and self.root.parent == p.CONTROL_ROOT, 'fresh exact transaction root required')
        self.root.parent.mkdir(mode=0o700, exist_ok=True)
        self.root.mkdir(mode=0o700)
        m.write_new(self.root / 'owner.json', {'schema': 'reservoir_production_owner_v1',
                    'transaction_id': self.plan['transaction_id'], 'actor': self.plan['actor'],
                    'host_binding': self.plan['host_binding'], 'preparation_sha256': p.preparation_digest(self.spec)})
        with (self.root / 'model_environment.py').open('xb') as handle:
            handle.write(p.HOLD_SOURCE.read_bytes()); handle.flush(); os.fsync(handle.fileno())
        self.anchors['reservoir'] = p.reservoir_anchor(self.io, self.plan['roles']['reservoir']['identity'])
        self.anchors['provider'] = p.provider_anchor(self.io, self.plan['roles']['provider']['identity'])
        for role in m.ROLES:
            m.write_new(self.root / (role + '-capture-anchor.json'), self.anchors[role])
        self._ownership()
        return {'schema': 'reservoir_production_capture_preparation_v1', 'passed': True,
                'anchors': self.anchors, 'execution_enabled': False, 'service_mutations': 0}

    def _allow(self):
        m.require(PRODUCTION_EXECUTION_ENABLED is True,
                  'production execution disabled: controller admission, exact review and real matrix remain required')
        self._ownership()
        review = p.reference(self.spec['execution_review'])
        matrix = p.reference(self.spec['real_matrix'])
        m.require(review.get('schema') == 'reservoir_production_execution_review_v1'
                  and review.get('approved') is True and review.get('preparation_sha256') == p.preparation_digest(self.spec)
                  and review.get('fixture') == self.spec['fixture'] and review.get('real_matrix') == self.spec['real_matrix'],
                  'exact production execution review missing')
        m.require(matrix.get('passed') is True and matrix.get('source_sha256') == self.plan['input_hashes']['reservoir_candidate']
                  and matrix.get('real_service_executed') is True, 'actual preservation/restore matrix missing')
        for path, digest in review.get('artifacts', {}).items():
            m.require(p.sha(path) == digest, 'reviewed production helper changed')
        required = {str(Path(__file__).resolve()), str(p.HERE / 'production_preflight.py'),
                    str(p.HERE / 'reservoir_maintenance.py'), str(p.HERE / 'reservoir_maintenance_state.py'),
                    str(p.HERE / 'restore_verifier.py'), str(p.HOLD_SOURCE)}
        m.require(required <= set(review.get('artifacts', {})), 'production review helper closure incomplete')
        owner = m.read_record(self.root / 'owner.json')
        m.require(owner.get('transaction_id') == self.plan['transaction_id']
                  and owner.get('preparation_sha256') == p.preparation_digest(self.spec)
                  and self.root == self.root.resolve() and self.root.stat().st_uid == os.getuid(), 'transaction owner changed')
        m.require(set(self.anchors) == set(m.ROLES), 'positive live capture anchors required')
        m.require(p.sha(self.root / 'model_environment.py') == self.spec['immutable_files'][str(p.HOLD_SOURCE)],
                  'copied holder source changed')

    def event(self, kind, **details):
        self.counter += 1
        m.write_new(self.root / f'host-operation-{self.counter:05d}.json',
                    {'operation': kind, 'monotonic': time.monotonic(), **details})

    def create_hold(self, role, guard):
        self._allow()
        m.require(guard == {'schema': 'model_environment_hold_v1', 'transaction_id': self.plan['transaction_id'],
                           'actor': self.plan['actor']}, 'wrong hold owner')
        self.event('hold_intent', role=role, guard=guard)
        result = self.env.create_hold(self.control(role), guard['transaction_id'], guard['actor'])
        self.guards[role] = result
        self.event('hold_created', role=role, guard=result)
        return {'guard': result}

    def arm_next_spawn(self, role, guard, expected):
        self._allow()
        m.require(os.geteuid() != 0 and sys.stdin.isatty(), 'ordinary-user Terminal required; never sudo controller')
        self.env.assert_hold(self.control(role), guard)
        view = self.inspect_role(role)
        m.require(view['process'] == expected and view.get('held') is None
                  and expected == self.plan['roles'][role]['identity'], 'original worker changed before arm')
        m.require(role not in self.armed, 'never repeat an override attempt automatically')
        self.armed.add(role)  # Includes an uncertain outcome; no retry in this Host.
        argv = ['/usr/bin/sudo', '--', '/bin/launchctl', 'debug', self.target(role), '--program',
                str(HOLDER_PYTHON), '--', *self.hold_argv(role)[1:]]
        self.event('debug_intent', role=role, command=argv, identity=expected)
        result = subprocess.run(argv, check=False, timeout=180)
        self.event('debug_result', role=role, returncode=result.returncode)
        m.require(result.returncode == 0, 'debug outcome not confirmed; hold retained, no signal')
        self.armed.add((role, expected['pid'], expected['started_at']))
        return {'armed': True}

    def signal_stop(self, role, expected):
        self._allow()
        key = (role, expected['pid'], expected['started_at'])
        m.require(key in self.armed and key not in self.signalled, 'signal lacks confirmed arm or was already attempted')
        m.require(self.inspect_role(role)['process'] == expected, 'process changed before cooperative signal')
        if role == 'reservoir':
            self._absent('provider')
            m.require(self.receipts.get('provider_drain', {}).get('passed') is True,
                      'positive final provider check-in witness required before reservoir stop')
        signal_time_ns = time.time_ns()
        self.event('signal_intent', role=role, identity=expected, signal='SIGTERM', wall_time_ns=signal_time_ns)
        self.signalled.add(key)
        self.signal_times[role] = signal_time_ns
        os.kill(expected['pid'], signal.SIGTERM)
        self.event('signal_sent', role=role, identity=expected)
        return {'signal': 'SIGTERM', 'identity': expected, 'force_used': False}

    def _absent(self, role):
        view = self.inspect_role(role)
        m.require(not view['job_loaded'] and view['process'] is None and not view['listener_pids']
                  and self.io.process(self.plan['roles'][role]['identity']['pid']) is None,
                  role + ' job, original process or listener survives')

    def remove_held(self, role, expected, guard):
        self._allow()
        view = self.inspect_role(role)
        m.require(view['process'] == expected and (view.get('held') or {}).get('guard') == guard
                  and not view['listener_pids'], 'bootout requires exact owned listener-free holder')
        m.require(self.io.process(self.plan['roles'][role]['identity']['pid']) is None, 'original worker survives beside holder')
        self.event('held_bootout_intent', role=role, identity=expected)
        result = self.io.command('/bin/launchctl', 'bootout', self.target(role))
        m.require(result.returncode == 0, 'held-only bootout failed; retain guard')
        wait_for(lambda: not self.inspect_role(role)['job_loaded'] and self.io.process(expected['pid']) is None,
                 'held job absence', 20)
        self._absent(role)
        self.event('held_job_absent', role=role, identity=expected)
        return {'removed_identity': expected}

    def _coverage(self, role, context):
        old = self.plan['roles'][role]['identity']
        m.require(self.io.process(old['pid']) is None, 'capture requires exact old process exit')
        view = self.inspect_role(role)
        m.require(view.get('held') is not None and not view['listener_pids'], 'capture requires replacement held')
        coverage = {'schema': 'reservoir_maintenance_log_coverage_v1', 'complete': True,
            'process_identity_sha256': context['bindings'][role + '_old_identity_sha256'], 'old_process_absent': True}
        anchor = self.anchors[role]
        coverage['log'] = p.capture_span(anchor['log'], self.root / (role + '-drain.log'))
        if role == 'provider':
            coverage['qos'] = p.capture_span(anchor['qos'], self.root / 'provider-drain-qos.jsonl')
            coverage.update(start_worker_idle=anchor['start_worker_idle'], start_queue_empty=anchor['start_queue_empty'])
        else:
            coverage['inventory_matches_start_boundary'] = anchor['inventory_matches_start_boundary']
        return coverage

    def collect_witness(self, kind, context):
        self._allow()
        import reservoir_maintenance_state as state
        if kind == 'provider_drain':
            coverage = self._coverage('provider', context)
            data, _ = p.stable_bytes(p.JOURNAL)
            journal = self.root / 'final-coupling-journal.json'
            with journal.open('xb') as handle:
                handle.write(data); handle.flush(); os.fsync(handle.fileno())
            receipt = state.verify_provider_drain(context, self.root / 'provider-drain.log',
                        self.root / 'provider-drain-qos.jsonl', coverage, journal)
        elif kind == 'final_snapshot':
            receipt = state.verify_final_snapshot(context, self.root / 'reservoir-drain.log',
                        self.anchors['reservoir']['inventory'], self._coverage('reservoir', context))
            for line in (self.root / 'reservoir-drain.log').read_text().splitlines():
                match = re.search(r"snapshot '([^']+)' → (.+)$", line)
                if match:
                    m.require(Path(match[2]) == p.STATE_ROOT / (match[1] + '.npz'),
                              'snapshot log names a different persistence path')
        elif kind == 'backup':
            self._absent('provider'); self._absent('reservoir')
            barrier = {'schema': 'reservoir_maintenance_stopped_barrier_v1', 'transaction_id': context['transaction_id'],
                       'bindings': context['bindings'], 'ownership_current': True,
                       **{role: {'job_absent': True, 'old_process_absent': True, 'listener_absent': True} for role in m.ROLES}}
            receipt = state.backup_stopped_tree(context, p.STATE_ROOT, self.root / 'stopped-backup', barrier)
            entries = receipt['evidence']['source_inventory']['entries']
            verify_final_files(entries, self.receipts['final_snapshot']['evidence']['final_handles'], self.signal_times['reservoir'])
            self._absent('provider'); self._absent('reservoir')
        elif kind == 'private_restore':
            self._absent('provider'); self._absent('reservoir')
            import restore_verifier
            m.require(self.restore_runner is not None, 'external reviewed real restore runner not integrated')
            p.validate_restore_inputs(self.spec, self.spec['restore_source_manifest'], self.spec['restore_runtime_manifest'])
            result_path = self.restore_runner(context, self.receipts['backup'], self.spec)
            receipt = restore_verifier.validate_restore_witness(Path(result_path), context, self.receipts['backup'])
            p.validate_restore_inputs(self.spec, receipt['evidence']['source_manifest'], receipt['evidence']['runtime_manifest'])
            self._absent('provider'); self._absent('reservoir')
        elif kind == 'startup':
            receipt = self._startup_witness(context, state)
        elif kind == 'provider_ready':
            view = self.inspect_role('provider'); health = self.io.health()
            m.require(view['process'] == self.startups['provider']['identity']
                      and view['listener_pids'] == [view['process']['pid']] and health.get('ready') is True
                      and health.get('worker', {}).get('phase') in ('ready', 'generating')
                      and health.get('worker', {}).get('last_generation_error') is None and not health.get('error')
                      and health.get('reservoir', {}).get('status') == 'connected', 'original provider readiness failed')
            receipt = state.envelope(context, kind, {'identity': view['process'], 'health': health,
                'journal_before_bootstrap': self.receipts['provider_drain']['evidence']['journal'],
                'limitation': 'provider admission resumed; newer journal state must never be rolled back'})
        else:
            raise m.Refusal('unknown witness kind')
        self.receipts[kind] = receipt
        m.write_new(self.root / (kind + '-witness.json'), receipt)
        return receipt

    def install_repair(self, context):
        self._allow(); self._absent('provider'); self._absent('reservoir')
        m.require(self.receipts.get('private_restore', {}).get('passed') is True, 'real restore acceptance required')
        target = p.SOURCES['reservoir']
        candidate = Path(self.spec['repair_source']['path'])
        data, _ = p.stable_bytes(candidate)
        m.require(p.sha(candidate) == self.plan['input_hashes']['reservoir_candidate']
                  and p.sha(target) == self.plan['input_hashes']['reservoir_source'], 'repair or original source changed')
        temporary = target.with_name(target.name + '.' + self.plan['transaction_id'] + '.tmp')
        self.event('install_intent', target=str(target), source_sha256=p.sha(candidate))
        with temporary.open('xb') as handle:
            handle.write(data); handle.flush(); os.fsync(handle.fileno())
        os.chmod(temporary, target.stat().st_mode & 0o777)
        self._absent('provider'); self._absent('reservoir')
        os.replace(temporary, target); m.sync_dir(target.parent)
        self.bound_stats[str(target)] = stamp(target)
        self.repaired = True
        self.event('install_completed', source_sha256=p.sha(target))
        return {'source_sha256': p.sha(target)}

    def bootstrap_original(self, role, context):
        self._allow(); self._absent(role)
        if role == 'reservoir':
            self._absent('provider')
            import reservoir_maintenance_state as state
            m.require(state.inventory_tree(p.STATE_ROOT) == self.receipts['backup']['evidence']['source_inventory'],
                      'stopped production state changed after backup/private restore')
        else:
            m.require(self.receipts.get('startup', {}).get('passed') is True, 'reservoir startup acceptance required')
            journal_ref = self.receipts['provider_drain']['evidence']['journal']
            m.require(p.sha(p.JOURNAL) == journal_ref['sha256'], 'coupling journal changed before original provider restart')
        original = Path(self.spec['originals'][role]['plist'])
        m.require(original.read_bytes() == p.PLISTS[role].read_bytes(), 'original launch settings changed')
        if role == 'provider':
            # The sole reviewed settings delta is the same checkpoint pinned
            # from hub ID to its immutable local snapshot. Keep old plist bytes.
            restart = self.spec['provider_restart_plist']
            model = p.reference(self.spec['original_provider_manifest'])
            snapshot = self.env.option(model['argv'], '--model')
            p.validate_provider_restart(self.spec, snapshot)
            data, _ = p.stable_bytes(Path(restart['path']))
            target = p.PLISTS['provider']
            temporary = target.with_name(target.name + '.' + self.plan['transaction_id'] + '.tmp')
            self.event('provider_snapshot_plist_intent', original_sha256=p.sha(original), restart_sha256=restart['sha256'])
            with temporary.open('xb') as handle:
                handle.write(data); handle.flush(); os.fsync(handle.fileno())
            os.chmod(temporary, target.stat().st_mode & 0o777)
            self._absent('provider')
            os.replace(temporary, target); m.sync_dir(target.parent)
            self.bound_stats[str(target)] = stamp(target)
            self.provider_pinned = True
            self.event('provider_snapshot_plist_installed', restart_sha256=p.sha(target))
        anchor = p.cursor(p.LOGS[role])
        self.event('bootstrap_intent', role=role, applied_plist_sha256=p.sha(p.PLISTS[role]),
                   original_rollback_plist_sha256=p.sha(original), log_anchor=anchor)
        result = self.io.command('/bin/launchctl', 'bootstrap', f"gui/{self.plan['host_binding']['uid']}", str(p.PLISTS[role]))
        m.require(result.returncode == 0, 'original bootstrap failed; no automatic rollback')
        expected_argv = (self.spec['provider_restart_argv'] if role == 'provider' else
                         self.plan['roles'][role]['identity']['argv'])
        def ready():
            view = self.inspect_role(role)
            ident = view.get('process')
            if ident is None:
                return None
            m.require(ident['argv'] == expected_argv and ident != self.plan['roles'][role]['identity'], 'replacement command or identity differs')
            if role == 'provider' and view['listener_pids'] == [ident['pid']]:
                health = self.io.health(); worker = health.get('worker', {})
                m.require(not health.get('error') and worker.get('phase') != 'failed', 'provider startup failed')
                if not (health.get('ready') is True and worker.get('phase') in ('ready', 'generating')
                        and worker.get('last_generation_error') is None
                        and health.get('reservoir', {}).get('status') == 'connected'):
                    return None
            return ident if view['listener_pids'] == [ident['pid']] else None
        identity = wait_for(ready, role + ' original readiness', self.plan['timeout_s'])
        self.startups[role] = {'identity': identity, 'log_anchor': anchor}
        self.event('bootstrap_ready', role=role, identity=identity)
        return {'identity': identity}

    def _startup_witness(self, context, state):
        started = self.startups['reservoir']
        m.require(self.inspect_role('reservoir')['process'] == started['identity'], 'reservoir changed during startup capture')
        path = self.root / 'reservoir-startup.log'
        span = p.capture_span(started['log_anchor'], path)
        restored = self.receipts['private_restore']['evidence']
        parsed = startup_log(path.read_bytes(), restored['expected_startup'], self.spec['reservoir_config'])
        m.require(self.inspect_role('reservoir')['process'] == started['identity'], 'reservoir changed after startup capture')
        return state.envelope(context, 'startup', {'identity': started['identity'], 'log': {'path': str(path), 'sha256': p.sha(path)},
            'coverage': span, **parsed, 'private_restore_sha256': m.digest(self.receipts['private_restore']),
            'method': 'identical stopped inputs/implementation plus complete startup logs; deterministic-path inference',
            'limitation': 'not direct live byte equality; background work begins after synchronous restoration'})

    def release_hold(self, role, guard):
        self._allow()
        m.require(self.receipts.get('provider_ready', {}).get('passed') is True, 'cannot release holds before complete acceptance')
        self.env.release_hold(self.control(role), guard)
        self.event('hold_released', role=role, guard=guard)
        return {'released': True}

    monotonic = staticmethod(time.monotonic)
    sleep = staticmethod(time.sleep)
