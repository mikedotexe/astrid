"""One finite stopped-backup restore child, callable only through the blocked Host.

No CLI, model imports, service calls, signals, retry or force-stop path exists.
The unchanged verifier owns its independent copied-state work directory.
"""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import time

import production_preflight as p
import restore_verifier as v

m = p.m
HERE = Path(__file__).resolve().parent
THREADS = ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'MKL_NUM_THREADS',
           'VECLIB_MAXIMUM_THREADS', 'NUMEXPR_NUM_THREADS', 'BLIS_NUM_THREADS')
TIMEOUT_S = 300


def child_environment():
    env = dict(os.environ)
    for key in ('PYTHONPATH', 'PYTHONHOME', 'PYTHONSTARTUP'):
        env.pop(key, None)
    env.update({key: '1' for key in THREADS})
    env.update(PYTHONDONTWRITEBYTECODE='1', PYTHONNOUSERSITE='1', TOKENIZERS_PARALLELISM='false')
    return env


class FiniteRestoreRunner:
    def __init__(self, host):
        self.host = host

    def _barrier(self):
        self.host._allow()
        self.host._absent('provider')
        self.host._absent('reservoir')

    def adopt_existing(self, context, backup, spec):
        """Read/adopt a fully recorded successful owned child; never relaunch it.

        Parked barriers are required before and after validation. A recorded
        natural reap is indispensable: absence of a process is not exit proof.
        """
        self._barrier()
        host = self.host
        m.require(host.scope == 'production' and spec == host.spec
                  and context['plan'] == host.plan
                  and context['transaction_id'] == host.plan['transaction_id']
                  and backup == host.receipts.get('backup'), 'restore belongs to another transaction')
        from production_recovery_inspect import Evidence, private_directory
        evidence = Evidence(); root = host.root
        m.require(root == Path(spec['transaction_root']), 'wrong retained restore root')
        private_directory(root)
        m.require(v.HERE == HERE and v.RUNS == HERE / 'restore-runs', 'foreign verifier import')
        unresolved = root / 'private-restore-unresolved.json'
        evidence.absent(unresolved)
        request_path = root / 'private-restore-request.json'
        request = evidence.read(request_path, record=True)
        python = spec['restore_runtime_manifest']['python']
        m.require(python == str(p.WORKSPACE / '.venv/bin/python'), 'exact original interpreter required')
        work = v.RUNS / ('production-' + host.plan['transaction_id'])
        argv = [python, '-B', str(HERE / 'restore_verifier.py'), '--request', str(request_path),
                '--execute-private-restore']
        binding = {'transaction_id': context['transaction_id'], 'plan_sha256': m.digest(host.plan),
                   'preparation_sha256': p.preparation_digest(spec),
                   'request': {'path': str(request_path), 'sha256': evidence.files[str(request_path)]['sha256']},
                   'backup_sha256': m.digest(backup), 'work_root': str(work), 'argv': argv,
                   'implementation': v.implementation()}
        records = {}
        for name, suffix in (('launch', 'launch'), ('start', 'child-start'), ('exit', 'child-exit')):
            row = evidence.read(root / ('private-restore-' + suffix + '.json'), record=True)
            schema = 'reservoir_production_restore_' + suffix.replace('-', '_') + '_v1'
            m.require(row.get('schema') == schema and all(row.get(k) == value for k, value in binding.items()),
                      'retained restore binding differs: ' + name)
            records[name] = row
        launch, start, exit_record = (records[name] for name in ('launch', 'start', 'exit'))
        m.require(launch.get('timeout_s') == TIMEOUT_S and launch.get('automatic_retry_permitted') is False,
                  'retained launch policy differs')
        m.require(type(start.get('pid')) is int and start['pid'] > 1 and start.get('signals_permitted') is False
                  and exit_record.get('pid') == start['pid'] and type(exit_record.get('returncode')) is int
                  and exit_record['returncode'] == 0 and exit_record.get('reaped') is True,
                  'clean owned naturally reaped restore exit required')
        times = [launch.get('started_wall'), start.get('started_wall'), exit_record.get('completed_wall')]
        m.require(all(v.finite(t) and t > 0 for t in times) and times == sorted(times),
                  'retained child execution clocks invalid')
        for stream in ('stdout', 'stderr'):
            m.require(exit_record.get(stream, {}).get('path') == str(root / ('private-restore-' + stream + '.txt')),
                      'retained child stream path differs')
            evidence.ref(exit_record[stream])
        m.require(request.get('context') == context and request.get('backup_witness') == backup
                  and request.get('source_manifest') == spec['restore_source_manifest']
                  and request.get('runtime_manifest') == spec['restore_runtime_manifest']
                  and request.get('config') == {k: spec['reservoir_config'][k] for k in ('input_dim', 'n_nodes', 'backend')}
                  and request.get('work_root') == str(work), 'retained request context/input differs')
        m.require(backup['evidence']['backup_root'] == str(root / 'stopped-backup' / 'state'),
                  'exact stopped backup required')
        p.validate_restore_inputs(spec, request['source_manifest'], request['runtime_manifest'])
        witness_path = work / 'restore-witness.json'
        witness = v.validate_restore_witness(witness_path, context, backup)
        evidence.read(witness_path, record=True)
        for name in ('request', 'comparison', 'log'): evidence.ref(witness['evidence'][name])
        m.require(witness['evidence']['request'] == binding['request'], 'witness belongs to another request')
        comparison = evidence.read(witness['evidence']['comparison']['path'], record=True)
        m.require(comparison['pid'] == start['pid'] and v.finite(comparison.get('started_wall'))
                  and v.finite(comparison.get('completed_wall'))
                  and times[0] <= comparison['started_wall'] <= comparison['completed_wall'] <= times[2],
                  'witness execution differs from owned child')
        p.validate_restore_inputs(spec, witness['evidence']['source_manifest'], witness['evidence']['runtime_manifest'])
        self._barrier(); evidence.finish()
        return witness_path

    def __call__(self, context, backup, spec):
        self._barrier()  # Includes the still-false production execution gate.
        host = self.host
        m.require(host.scope == 'production' and spec == host.spec
                  and context['plan'] == host.plan
                  and context['transaction_id'] == host.plan['transaction_id']
                  and backup == host.receipts.get('backup'), 'restore belongs to another transaction')
        root = host.root
        m.require(root == Path(spec['transaction_root']) and root == root.resolve()
                  and root.stat().st_uid == os.getuid(), 'owned transaction root required')
        m.require(not any((root / name).exists() for name in (
            'private-restore-launch.json', 'private-restore-request.json',
            'private-restore-child-start.json', 'private-restore-child-exit.json',
            'private-restore-unresolved.json')), 'restore was already prepared or attempted; never retry')
        m.require(v.HERE == HERE and v.RUNS == HERE / 'restore-runs', 'foreign verifier import')
        python = spec['restore_runtime_manifest']['python']
        m.require(python == str(p.WORKSPACE / '.venv/bin/python'), 'exact original interpreter required')
        backup_root = root / 'stopped-backup' / 'state'
        m.require(backup['evidence']['backup_root'] == str(backup_root)
                  and backup_root == backup_root.resolve(strict=True), 'exact stopped backup required')
        p.validate_restore_inputs(spec, spec['restore_source_manifest'], spec['restore_runtime_manifest'])
        config = {key: spec['reservoir_config'][key] for key in ('input_dim', 'n_nodes', 'backend')}
        work = v.RUNS / ('production-' + host.plan['transaction_id'])
        m.require(not work.exists(), 'restore work already exists; never retry')
        request = v.prepare_request(context, backup, source_manifest=spec['restore_source_manifest'],
                                    runtime_manifest=spec['restore_runtime_manifest'], config=config, work_root=work)
        v.check_inputs(request)
        v.check_runtime(request, current=False)
        self._barrier()
        request_path = root / 'private-restore-request.json'
        m.write_new(request_path, request)
        argv = [python, '-B', str(HERE / 'restore_verifier.py'), '--request', str(request_path),
                '--execute-private-restore']
        binding = {'transaction_id': context['transaction_id'], 'plan_sha256': m.digest(host.plan),
                   'preparation_sha256': p.preparation_digest(spec), 'request': v.reference(request_path),
                   'backup_sha256': m.digest(backup), 'work_root': str(work), 'argv': argv,
                   'implementation': v.implementation()}
        # Persist intent before Popen. Even an uncertain launch cannot be repeated.
        m.write_new(root / 'private-restore-launch.json', {
            'schema': 'reservoir_production_restore_launch_v1', **binding,
            'started_wall': time.time(), 'timeout_s': TIMEOUT_S, 'automatic_retry_permitted': False})
        process = None
        try:
            out, err = root / 'private-restore-stdout.txt', root / 'private-restore-stderr.txt'
            with out.open('xb') as stdout, err.open('xb') as stderr:
                process = subprocess.Popen(argv, cwd=str(root), env=child_environment(),
                                           stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                           start_new_session=True)
                m.write_new(root / 'private-restore-child-start.json', {
                    'schema': 'reservoir_production_restore_child_start_v1', **binding,
                    'pid': process.pid, 'started_wall': time.time(), 'signals_permitted': False})
                code = process.wait(timeout=TIMEOUT_S)
            m.write_new(root / 'private-restore-child-exit.json', {
                'schema': 'reservoir_production_restore_child_exit_v1', **binding,
                'pid': process.pid, 'returncode': code, 'completed_wall': time.time(), 'reaped': True,
                'stdout': v.reference(out), 'stderr': v.reference(err)})
            m.require(code == 0, 'private restore child failed; retain evidence, no retry')
            self._barrier()
            m.require(v.reference(request_path) == binding['request'], 'restore request changed')
            witness_path = work / 'restore-witness.json'
            witness = v.validate_restore_witness(witness_path, context, backup)
            comparison = v.read_json(witness['evidence']['comparison']['path'])
            m.require(comparison['pid'] == process.pid, 'restore witness was not produced by this owned child')
            p.validate_restore_inputs(spec, witness['evidence']['source_manifest'], witness['evidence']['runtime_manifest'])
            self._barrier()
            return witness_path
        except BaseException as error:
            # Never signal even if the worker outlives this caller. A missing
            # child-start record makes identity uncertain, not retryable.
            try:
                alive = None if process is None else process.poll() is None
            except BaseException:
                alive = None
            m.write_new(root / 'private-restore-unresolved.json', {
                'schema': 'reservoir_production_restore_unresolved_v1', **binding,
                'pid': None if process is None else process.pid,
                'error': type(error).__name__ + ': ' + str(error), 'may_still_be_running': alive,
                'automatic_retry_permitted': False, 'next_child_permitted': False,
                'force_used': False, 'production_install_permitted': False})
            raise
