"""Finite restore orchestration with fake children only; never loads NumPy."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import types
import time
import unittest
from unittest.mock import Mock, patch

import production_restore_runner as r
import production_host as h


class RestoreRunnerTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix='stopped-restore-fake-')
        self.addCleanup(tmp.cleanup)
        self.top = Path(tmp.name).resolve(); self.root = self.top / 'transaction'; self.root.mkdir(mode=0o700)
        self.lib = self.top / 'lib'; self.lib.mkdir(); (self.lib / 'restore_verifier.py').write_text('# never executed\n')
        self.workspace = self.top / 'runtime'; self.backup = self.root / 'stopped-backup' / 'state'; self.backup.mkdir(parents=True)
        self.plan = {'transaction_id': 'a' * 32}
        self.context = {'transaction_id': self.plan['transaction_id'], 'plan': self.plan}
        self.backup_witness = {'passed': True, 'evidence': {'backup_root': str(self.backup)}}
        self.spec = {'transaction_root': str(self.root),
                     'restore_source_manifest': {'root': str(self.lib)},
                     'restore_runtime_manifest': {'python': str(self.workspace / '.venv/bin/python')},
                     'reservoir_config': {'input_dim': 32, 'n_nodes': 192, 'backend': 'numpy', 'backend_label': 'NumPy'}}
        self.host = types.SimpleNamespace(scope='production', spec=self.spec, plan=self.plan, root=self.root,
                    receipts={'backup': self.backup_witness}, _allow=Mock(), _absent=Mock())
        self.runner = r.FiniteRestoreRunner(self.host)
        self.work = self.lib / 'restore-runs' / ('production-' + self.plan['transaction_id'])
        self.child = Mock(pid=12345); self.child.poll.return_value = 0
        self.patches = []
        def patcher(target, name, **kwargs):
            item = patch.object(target, name, **kwargs); self.patches.append(item); return item.start()
        patcher(r, 'HERE', new=self.lib); patcher(r.v, 'HERE', new=self.lib)
        patcher(r.v, 'RUNS', new=self.lib / 'restore-runs'); patcher(r.p, 'WORKSPACE', new=self.workspace)
        self.inputs = patcher(r.p, 'validate_restore_inputs', return_value=None)
        self.prepare = patcher(r.v, 'prepare_request', side_effect=lambda *a, **k: {
            'fixture': True, 'context': a[0], 'backup_witness': a[1], 'config': k['config'],
            'source_manifest': k['source_manifest'], 'runtime_manifest': k['runtime_manifest'], 'work_root': str(k['work_root'])})
        patcher(r.v, 'check_inputs', return_value=None); patcher(r.v, 'check_runtime', return_value=None)
        patcher(r.v, 'implementation', return_value={'fake-verifier': 'a' * 64})
        self.validate = patcher(r.v, 'validate_restore_witness', side_effect=self.witness)
        self.spawn = patcher(r.subprocess, 'Popen', return_value=self.child)
        self.child.wait.side_effect = self.finish
        self.addCleanup(lambda: [p.stop() for p in reversed(self.patches)])

    def finish(self, **kwargs):
        self.work.mkdir(parents=True)
        (self.work / 'comparison.json').write_text(json.dumps({'pid': self.child.pid, 'started_wall': time.time(), 'completed_wall': time.time()}))
        (self.work / 'restore.log').write_text('synthetic fixture restore log\n')
        (self.work / 'restore-witness.json').write_text('{}')
        return 0

    def witness(self, path, context, backup):
        self.assertEqual(path, self.work / 'restore-witness.json')
        self.assertTrue(path.is_file())
        self.assertEqual(context, self.context); self.assertEqual(backup, self.backup_witness)
        return {'evidence': {'comparison': r.v.reference(self.work / 'comparison.json'),
                            'request': r.v.reference(self.root / 'private-restore-request.json'),
                            'log': r.v.reference(self.work / 'restore.log'),
                            'source_manifest': self.spec['restore_source_manifest'],
                            'runtime_manifest': self.spec['restore_runtime_manifest']}}

    def run_restore(self):
        return self.runner(self.context, self.backup_witness, self.spec)

    def test_default_production_gate_refuses_before_file_or_child(self):
        blocked = h.ProductionHost.__new__(h.ProductionHost)
        self.runner.host = blocked
        with patch.object(h,'PRODUCTION_EXECUTION_ENABLED',False), self.assertRaisesRegex(h.m.Refusal, 'execution disabled'):
            self.run_restore()
        self.spawn.assert_not_called()
        self.assertEqual(list(self.root.iterdir()), [self.backup.parent])

    def test_exact_launch_and_natural_reap_bind_owned_result(self):
        self.assertEqual(self.run_restore(), self.work / 'restore-witness.json')
        argv = self.spawn.call_args.args[0]
        self.assertEqual(argv, [self.spec['restore_runtime_manifest']['python'], '-B',
            str(self.lib / 'restore_verifier.py'), '--request', str(self.root / 'private-restore-request.json'),
            '--execute-private-restore'])
        options = self.spawn.call_args.kwargs
        self.assertEqual(options['cwd'], str(self.root)); self.assertEqual(options['stdin'], subprocess.DEVNULL)
        self.assertTrue(options['start_new_session'])
        self.assertTrue(all(options['env'][key] == '1' for key in r.THREADS))
        self.assertEqual(self.prepare.call_args.kwargs['config'], {'input_dim': 32, 'n_nodes': 192, 'backend': 'numpy'})
        self.child.wait.assert_called_once_with(timeout=300)
        result = json.loads((self.root / 'private-restore-child-exit.json').read_text())
        self.assertTrue(result['reaped']); self.assertEqual(result['pid'], self.child.pid)
        self.assertEqual(result['returncode'], 0); self.assertEqual(self.inputs.call_count, 2)
        self.assertEqual(self.host._absent.call_count, 8)
        self.child.kill.assert_not_called(); self.child.terminate.assert_not_called()

    def test_no_second_run_even_after_success(self):
        self.run_restore()
        with self.assertRaisesRegex(r.m.Refusal, 'never retry'): self.run_restore()
        self.spawn.assert_called_once()

    def test_foreign_interpreter_refused_before_request_or_spawn(self):
        self.spec['restore_runtime_manifest']['python'] = '/foreign/python'
        with self.assertRaisesRegex(r.m.Refusal, 'exact original'): self.run_restore()
        self.spawn.assert_not_called(); self.assertFalse((self.root / 'private-restore-request.json').exists())

    def test_foreign_backup_and_source_refused(self):
        self.backup_witness['evidence']['backup_root'] = str(self.top)
        with self.assertRaisesRegex(r.m.Refusal, 'exact stopped backup'): self.run_restore()
        self.spawn.assert_not_called()
        self.backup_witness['evidence']['backup_root'] = str(self.backup)
        self.inputs.side_effect = r.m.Refusal('private source changed')
        with self.assertRaisesRegex(r.m.Refusal, 'private source'): self.run_restore()
        self.spawn.assert_not_called()

    def test_actual_backup_helper_layout_is_accepted(self):
        # Exercise the real stdlib backup envelope, not a duplicated layout.
        self.backup.rmdir(); self.backup.parent.rmdir()
        source = self.top / 'synthetic-source'; source.mkdir()
        (source / 'history.txt').write_text('preserved synthetic history')
        self.context['bindings'] = {key: 'a' * 64 for key in (*r.v.state.COMMON, 'final_snapshot_sha256')}
        barrier = {'schema': 'reservoir_maintenance_stopped_barrier_v1',
                   'transaction_id': self.context['transaction_id'], 'bindings': self.context['bindings'],
                   'ownership_current': True,
                   **{role: {'job_absent': True, 'old_process_absent': True, 'listener_absent': True}
                      for role in ('provider', 'reservoir')}}
        self.backup_witness = r.v.state.backup_stopped_tree(self.context, source,
                                                          self.root / 'stopped-backup', barrier)
        self.host.receipts['backup'] = self.backup_witness
        self.run_restore()
        self.assertEqual(Path(self.backup_witness['evidence']['backup_root']), self.backup)
        self.assertEqual((source / 'history.txt').read_text(), 'preserved synthetic history')

    def test_existing_work_or_foreign_verifier_refused(self):
        self.work.mkdir(parents=True)
        with self.assertRaisesRegex(r.m.Refusal, 'work already exists'): self.run_restore()
        self.spawn.assert_not_called()
        with patch.object(r.v, 'HERE', self.top), self.assertRaisesRegex(r.m.Refusal, 'foreign verifier'):
            self.run_restore()

    def test_nonzero_exit_retains_failed_record_and_blocks_retry(self):
        self.child.wait.side_effect = None; self.child.wait.return_value = 1
        with self.assertRaisesRegex(r.m.Refusal, 'child failed'): self.run_restore()
        self.validate.assert_not_called()
        self.assertTrue((self.root / 'private-restore-child-exit.json').exists())
        with self.assertRaisesRegex(r.m.Refusal, 'never retry'): self.run_restore()
        self.spawn.assert_called_once()

    def test_timeout_retains_owned_child_and_no_force(self):
        self.child.wait.side_effect = subprocess.TimeoutExpired('private fixture', 300)
        self.child.poll.return_value = None
        with self.assertRaises(subprocess.TimeoutExpired): self.run_restore()
        row = json.loads((self.root / 'private-restore-unresolved.json').read_text())
        self.assertTrue(row['may_still_be_running']); self.assertFalse(row['force_used'])
        self.assertFalse(row['next_child_permitted']); self.assertFalse(row['production_install_permitted'])
        self.child.kill.assert_not_called(); self.child.terminate.assert_not_called()
        self.validate.assert_not_called()

    def test_interruption_and_uncertain_launch_keep_intent(self):
        self.spawn.side_effect = KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt): self.run_restore()
        row = json.loads((self.root / 'private-restore-unresolved.json').read_text())
        self.assertIsNone(row['pid']); self.assertIsNone(row['may_still_be_running'])
        self.assertFalse(row['automatic_retry_permitted'])
        self.assertTrue((self.root / 'private-restore-launch.json').exists())
        with self.assertRaisesRegex(r.m.Refusal, 'never retry'): self.run_restore()
        self.spawn.assert_called_once()

    def test_unrelated_witness_pid_is_rejected(self):
        def finish(**kwargs):
            self.finish(**kwargs); (self.work / 'comparison.json').write_text('{"pid":9999}'); return 0
        self.child.wait.side_effect = finish
        with self.assertRaisesRegex(r.m.Refusal, 'this owned child'): self.run_restore()
        self.assertTrue((self.root / 'private-restore-unresolved.json').exists())

    def test_missing_or_invalid_witness_never_accepts(self):
        self.validate.side_effect = ValueError('witness changed')
        with self.assertRaisesRegex(ValueError, 'witness changed'): self.run_restore()
        self.assertTrue((self.root / 'private-restore-unresolved.json').exists())

    def test_post_child_service_barrier_failure_blocks_acceptance(self):
        self.host._allow.side_effect = [None, None, r.m.Refusal('service appeared')]
        with self.assertRaisesRegex(r.m.Refusal, 'service appeared'): self.run_restore()
        self.validate.assert_not_called()
        self.assertTrue((self.root / 'private-restore-child-exit.json').exists())

    def test_post_child_input_drift_blocks_acceptance(self):
        self.inputs.side_effect = [None, r.m.Refusal('source changed during restore')]
        with self.assertRaisesRegex(r.m.Refusal, 'during restore'): self.run_restore()
        self.assertTrue((self.root / 'private-restore-unresolved.json').exists())

    def test_adopt_complete_success_without_second_child_or_producer(self):
        self.run_restore(); self.spawn.reset_mock(); self.prepare.reset_mock()
        before = {str(path): path.read_bytes() for path in self.root.rglob('*') if path.is_file()}
        self.assertEqual(self.runner.adopt_existing(self.context,self.backup_witness,self.spec), self.work/'restore-witness.json')
        self.spawn.assert_not_called();self.prepare.assert_not_called()
        self.assertEqual(before,{str(path):path.read_bytes() for path in self.root.rglob('*') if path.is_file()})

    def test_adopt_missing_or_unreaped_exit_cannot_infer_process_success(self):
        self.run_restore(); self.spawn.reset_mock()
        path=self.root/'private-restore-child-exit.json'; original=path.read_bytes()
        path.unlink()
        with self.assertRaises(FileNotFoundError):self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        path.write_bytes(original);row=json.loads(original);row['reaped']=False;path.write_text(json.dumps(row))
        with self.assertRaisesRegex(r.m.Refusal,'naturally reaped'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        self.spawn.assert_not_called()

    def test_adopt_rejects_uncertain_marker_changed_request_and_other_child(self):
        self.run_restore();self.spawn.reset_mock()
        marker=self.root/'private-restore-unresolved.json';marker.write_text('{}')
        with self.assertRaisesRegex(r.m.Refusal,'absent artifact'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        marker.unlink()
        path=self.root/'private-restore-request.json';data=path.read_bytes();path.write_bytes(data+b' ')
        with self.assertRaisesRegex(r.m.Refusal,'binding differs'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        path.write_bytes(data)
        comparison=self.work/'comparison.json';row=json.loads(comparison.read_text());row['pid']=99999;comparison.write_text(json.dumps(row))
        with self.assertRaisesRegex(r.m.Refusal,'owned child'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        self.spawn.assert_not_called()

    def test_adopt_requires_both_parked_barriers_and_same_runtime(self):
        self.run_restore();self.spawn.reset_mock();self.host._absent.reset_mock()
        self.host._absent.side_effect=r.m.Refusal('job appeared')
        with self.assertRaisesRegex(r.m.Refusal,'job appeared'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        self.host._absent.side_effect=None
        self.inputs.side_effect=r.m.Refusal('runtime drift')
        with self.assertRaisesRegex(r.m.Refusal,'runtime drift'):
            self.runner.adopt_existing(self.context,self.backup_witness,self.spec)
        self.spawn.assert_not_called()

    def test_child_environment_removes_python_overrides(self):
        with patch.dict(os.environ, {'PYTHONPATH': '/foreign', 'PYTHONHOME': '/foreign', 'PYTHONSTARTUP': '/foreign'}):
            env = r.child_environment()
        self.assertFalse({'PYTHONPATH', 'PYTHONHOME', 'PYTHONSTARTUP'} & env.keys())


if __name__ == '__main__': unittest.main(verbosity=2)
