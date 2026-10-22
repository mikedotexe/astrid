"""Tiny file/fake observation tests; no live probes, imports or service changes."""
import hashlib
import json
import os
from pathlib import Path
import plistlib
import tempfile
import types
import subprocess
import unittest
import copy
from unittest.mock import Mock, patch

import production_preflight as p


class PreflightTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix='production-preflight-fake-')
        self.addCleanup(tmp.cleanup); self.root = Path(tmp.name).resolve()
        self.log = self.root / 'provider.log'; self.log.write_bytes(b'old\n')
        self.reservoir = self.root / 'reservoir.log'; self.reservoir.write_bytes(b'older\n')
        self.qos = self.root / 'qos.jsonl'; self.qos.write_bytes(b'')
        for name, value in [('LOGS', {'provider': self.log, 'reservoir': self.reservoir}), ('QOS', self.qos)]:
            handle = patch.object(p, name, value); handle.start(); self.addCleanup(handle.stop)
        forbidden = patch.object(p.subprocess, 'run', side_effect=AssertionError('external command forbidden'))
        self.run = forbidden.start(); self.addCleanup(forbidden.stop)
        self.ident = {'pid': 123, 'started_at': 'start', 'argv': ['python', 'server.py']}
        self.io = Mock()
        self.io.role.return_value = {'process': self.ident}
        self.io.health.return_value = {'model': 'coupled-astrid', 'ready': True,
            'worker': {'phase': 'ready', 'queue_depth': 0, 'last_generation_error': None},
            'reservoir': {'status': 'connected'}}
        self.io.list_handles.return_value = {'type': 'list_handles_response', 'handles': [{'name': 'astrid'}]}

    def test_native_engine_binding_is_distinct_and_complete(self):
        spec = {'immutable_files': {str(path): 'a' * 64 for path in p.ENGINE_INPUTS},
                'plan': {'protected': {'minime_engine': {'pid': 123, 'started_at': 'start',
                         'argv': [str(p.MINIME / 'minime/target/release/minime'), 'run']}}}}
        p.validate_engine_inputs(spec)
        changed = copy.deepcopy(spec); changed['plan']['protected']['minime_engine']['argv'][0] = '/python/autonomous_agent.py'
        with self.assertRaisesRegex(p.m.Refusal, 'native Minime engine command'): p.validate_engine_inputs(changed)
        for path in p.ENGINE_INPUTS:
            changed = copy.deepcopy(spec); del changed['immutable_files'][str(path)]
            with self.subTest(path=path), self.assertRaisesRegex(p.m.Refusal, 'closure missing'): p.validate_engine_inputs(changed)

    def test_production_inventory_keeps_all_ten_distinct_processes(self):
        plan = {'protected': {name: dict(self.ident, pid=100+index) for index, name in enumerate(sorted(p.PROTECTED))},
                'roles': {name: {'identity': dict(self.ident, pid=200+index)} for index, name in enumerate(p.m.ROLES)}}
        p.validate_process_inventory(plan)
        duplicate_protected = copy.deepcopy(plan)
        duplicate_protected['protected']['astrid_feeder'] = duplicate_protected['protected']['collab_feeder']
        duplicate_managed = copy.deepcopy(plan)
        duplicate_managed['protected']['astrid_feeder'] = duplicate_managed['roles']['provider']['identity']
        for changed in (duplicate_protected, duplicate_managed):
            with self.subTest(plan=changed), self.assertRaisesRegex(p.m.Refusal, 'ten distinct processes'):
                p.validate_process_inventory(changed)
        incomplete = copy.deepcopy(plan); del incomplete['protected']['astrid_feeder']
        with self.assertRaisesRegex(p.m.Refusal, 'eight-process protected'): p.validate_process_inventory(incomplete)

    def test_large_prefix_is_streamed_even_when_tail_bound_is_tiny(self):
        # 3MiB stands in for the actual 2.7GiB prefix; read calls are <=1MiB,
        # and the bounded tail limit deliberately remains only 64 bytes.
        data = b'x' * (3 * 1024 * 1024 - 1) + b'\n'
        self.reservoir.write_bytes(data)
        original = os.read; sizes = []
        def traced(fd, size):
            sizes.append(size); return original(fd, size)
        with patch.object(p.os, 'read', side_effect=traced), patch.object(p, 'MAX_CAPTURE', 64):
            anchor = p.cursor(self.reservoir)
            with self.reservoir.open('ab') as handle: handle.write(b'new tail\n')
            span = p.capture_span(anchor, self.root / 'tail.log')
        self.assertEqual(anchor['prefix_sha256'], hashlib.sha256(data).hexdigest())
        self.assertEqual((self.root / 'tail.log').read_bytes(), b'new tail\n')
        self.assertEqual(span['end_offset'] - span['start_offset'], 9)
        self.assertLessEqual(max(sizes), 1024 * 1024)

    def test_append_during_prefix_hash_is_not_read_into_anchor(self):
        original = os.read; appended = False
        def append(fd, size):
            nonlocal appended
            if not appended:
                appended = True
                with self.log.open('ab') as handle: handle.write(b'next\n')
            return original(fd, size)
        with patch.object(p.os, 'read', side_effect=append): anchor = p.cursor(self.log)
        self.assertEqual(anchor['start_offset'], 4)
        self.assertEqual(anchor['prefix_sha256'], hashlib.sha256(b'old\n').hexdigest())

    def test_truncated_rewritten_rotated_and_partial_logs_refuse(self):
        for case in ('truncate', 'rewrite', 'rotate', 'partial'):
            self.log.write_bytes(b'old\n'); anchor = p.cursor(self.log)
            if case == 'truncate': self.log.write_bytes(b'')
            if case == 'rewrite': self.log.write_bytes(b'new\n')
            if case == 'rotate': self.log.unlink(); self.log.write_bytes(b'new\n')
            if case == 'partial':
                with self.log.open('ab') as handle: handle.write(b'partial')
            with self.subTest(case=case), self.assertRaises(p.m.Refusal):
                p.capture_span(anchor, self.root / (case + '.log'))

    def test_provider_positive_idle_anchor_and_exact_identity(self):
        result = p.provider_anchor(self.io, self.ident)
        self.assertTrue(result['start_worker_idle']); self.assertTrue(result['start_queue_empty'])
        self.assertEqual(self.io.health.call_count, 2)
        self.io.role.return_value = {'process': dict(self.ident, pid=456)}
        with self.assertRaises(p.m.Refusal): p.provider_anchor(self.io, self.ident)

    def test_provider_activity_or_queue_during_anchor_refuses(self):
        self.io.health.return_value['worker']['queue_depth'] = 1
        with self.assertRaises(p.m.Refusal): p.provider_anchor(self.io, self.ident)
        self.io.health.return_value['worker']['queue_depth'] = 0
        health = self.io.health.return_value
        def activity():
            with self.qos.open('ab') as handle: handle.write(b'activity\n')
            return health
        self.io.health.side_effect = activity
        with self.assertRaises(p.m.Refusal): p.provider_anchor(self.io, self.ident)

    def test_reservoir_anchor_rejects_lifecycle_crossing_inventory(self):
        inventory = self.io.list_handles.return_value
        def crossing():
            with self.reservoir.open('ab') as handle: handle.write(b"created handle 'later'\n")
            return inventory
        self.io.list_handles.side_effect = crossing
        with self.assertRaisesRegex(p.m.Refusal, 'lifecycle'): p.reservoir_anchor(self.io, self.ident)

    def test_reservoir_short_reads_are_completed(self):
        inventory = self.io.list_handles.return_value
        def append():
            with self.reservoir.open('ab') as handle: handle.write(b'ordinary status\n')
            return inventory
        self.io.list_handles.side_effect = append
        original = os.read
        with patch.object(p.os, 'read', side_effect=lambda fd, size: original(fd, min(size, 3))):
            result = p.reservoir_anchor(self.io, self.ident)
        self.assertTrue(result['inventory_matches_start_boundary'])

    def test_symlink_log_and_excess_tail_refuse(self):
        alias = self.root / 'alias'; alias.symlink_to(self.log)
        with self.assertRaises(OSError): p.cursor(alias)
        anchor = p.cursor(self.log)
        with self.log.open('ab') as handle: handle.write(b'x'*65 + b'\n')
        with patch.object(p, 'MAX_CAPTURE', 64), self.assertRaises(p.m.Refusal):
            p.capture_span(anchor, self.root / 'too-big')

    def test_approval_hash_projection_is_not_circular(self):
        spec = {'schema': 'example', 'plan': {'a': 1}}
        before = p.preparation_digest(spec)
        spec['execution_review'] = {'path': '/review', 'sha256': 'a'*64}
        self.assertEqual(before, p.preparation_digest(spec))
        spec['plan']['a'] = 2
        self.assertNotEqual(before, p.preparation_digest(spec))

    def test_config_is_derived_from_original_argv(self):
        expected = {'input_dim': 32, 'n_nodes': 192, 'backend': 'numpy', 'backend_label': 'NumPy'}
        self.assertEqual(p.reservoir_config(['python', 'service.py']), expected)
        for args in (['--backend', 'mlx'], ['--port', '9999'], ['--state-dir', '/foreign'], ['--port', '7881', '--port', '7881']):
            with self.subTest(args=args), self.assertRaises(p.m.Refusal): p.reservoir_config(args)

    def test_default_check_performs_no_service_probe_or_output_write(self):
        plan = {'host_binding': {'uid': 501}, 'pause_generation': 1, 'actor': 'owner',
                'protected': {'other': self.ident}, 'roles': {role: {'identity': self.ident} for role in p.m.ROLES}}
        self.io.host.return_value = plan['host_binding']
        self.io.stewardship.return_value = {'paused': True, 'pause_generation': 1, 'pause_actor': 'owner', 'lease': None}
        self.io.process.return_value = self.ident
        self.io.role.return_value = {'job_loaded': True, 'process': self.ident, 'listener_pids': [123]}
        plists = {}
        for role in p.m.ROLES:
            path = self.root / (role + '.plist'); path.write_bytes(plistlib.dumps({'ProgramArguments': self.ident['argv']})); plists[role] = path
        before = sorted(self.root.iterdir())
        with patch.object(p, 'validate_preparation', return_value=plan), patch.object(p, 'PLISTS', plists), \
             patch.object(p, 'load_hold', return_value=types.SimpleNamespace(command_matches=lambda a,b:a==b)):
            result = p.check_only({}, self.io)
        self.assertTrue(result['passed']); self.assertFalse(result['execution_enabled'])
        self.io.health.assert_not_called(); self.io.list_handles.assert_not_called()
        self.assertEqual(before, sorted(self.root.iterdir())); self.run.assert_not_called()

    def test_provider_pin_changes_only_model_value_and_keeps_wrapper_alias(self):
        old = {'Label': 'label', 'ProgramArguments': ['/venv/bin/python', '/service.py', '--model', 'hub/name', '--port', '8090'],
               'WorkingDirectory': '/cwd', 'KeepAlive': True}
        original = self.root / 'original.plist'; original.write_bytes(plistlib.dumps(old))
        pinned = dict(old, ProgramArguments=p.pinned_argv(old['ProgramArguments'], '/exact/snapshot'))
        path = self.root / 'pinned.plist'; path.write_bytes(plistlib.dumps(pinned))
        process = ['/framework/Python', *old['ProgramArguments'][1:]]
        spec = {'plan': {'roles': {'provider': {'identity': {'argv': process}}}},
                'provider_restart_argv': p.pinned_argv(process, '/exact/snapshot'),
                'provider_restart_plist': {'path': str(path), 'sha256': p.sha(path)}}
        with patch.object(p, 'PLISTS', {'provider': original}):
            p.validate_provider_restart(spec, '/exact/snapshot')
            changed = copy.deepcopy(spec); changed['provider_restart_argv'][-1] = '9999'
            with self.assertRaises(p.m.Refusal): p.validate_provider_restart(changed, '/exact/snapshot')
            pinned['KeepAlive'] = False; path.write_bytes(plistlib.dumps(pinned))
            spec['provider_restart_plist']['sha256'] = p.sha(path)
            with self.assertRaises(p.m.Refusal): p.validate_provider_restart(spec, '/exact/snapshot')

    def closure_fixture(self):
        workspace = self.root / 'workspace'; workspace.mkdir()
        frozen = self.root / 'frozen'; frozen.mkdir()
        restore = self.root / 'restore'; restore.mkdir()
        files = {}
        for name in p.RESTORE_NAMES:
            data = (name+' original\n').encode()
            for directory in (workspace, frozen, restore): (directory/name).write_bytes(data)
            files[str(workspace/name)] = p.sha(workspace/name)
        (restore/'reservoir_service.py').write_bytes(b'repaired scheduler\n')
        artifacts = {str(path): p.sha(path) for path in frozen.glob('*.py')}
        venv = workspace/'.venv'; venv.mkdir()
        for name in ('pyvenv.cfg', 'METADATA', 'RECORD'): (venv/name).write_text(name)
        artifacts.update({str(path):p.sha(path) for path in venv.iterdir()})
        inventory = self.root/'inventory.json'; inventory.write_text('{}')
        artifacts[str(inventory)] = p.sha(inventory)
        interpreter = self.root/'interpreter'; interpreter.write_text('python')
        artifacts[str(interpreter)] = p.sha(interpreter)
        model = self.root/'model'; model.mkdir()
        for name in ('config.json', 'model.safetensors'): (model/name).write_text(name)
        artifacts.update({str(path):p.sha(path) for path in model.iterdir()})
        files.update(artifacts)
        manifest = {'python':str(venv/'bin/python'),'working_directory':str(workspace),
                    'source_root':str(frozen),'artifacts':artifacts,'package_inventory':str(inventory),'packages':{'numpy':'1'}}
        manifest_path=self.root/'manifest.json'; manifest_path.write_text(json.dumps(manifest))
        files[str(manifest_path)]=p.sha(manifest_path)
        holder = self.root/'holder-python'; holder.write_text('holder'); files[str(holder)]=p.sha(holder)
        helpers={str(p.HOLD_SOURCE),*[str(p.HERE/name) for name in ['production_host.py','production_preflight.py',
                 'reservoir_maintenance.py','reservoir_maintenance_state.py','restore_verifier.py','production_admission.py','production_restore_admission.py',
                 'production_fixture_admission.py','production_fixture_history.py','production_fixture_native.py','production_fixture_readiness.py',
                 'production_native.py','production_native_attempt.py','production_entrypoint.py',
                 'production_restore_runner.py','production_recovery.py','production_recovery_inspect.py','production_retained.py',
                 'transaction_execution.py','recovery_ledger.py','recovery_phases.py','production_recovery_driver.py']]}
        spec={'immutable_files':files,'original_provider_manifest':{'path':str(manifest_path),'sha256':p.sha(manifest_path)},
              'input_groups':{'interpreter':[str(interpreter)],'model_snapshot':[str(path) for path in model.iterdir()],
               'provider_packages':[str(path) for path in venv.iterdir()]+[str(inventory)],'helpers':list(helpers)},
              'plan':{'host_binding':{},'roles':{'provider':{'identity':{'argv':['python','server','--model','hub']}},
               'reservoir':{'identity':{'argv':['python','service']}}}},'fixture':{'path':'/fixture'},'model_resolution':{'path':'/resolution'},
              'reservoir_config':{'input_dim':32,'n_nodes':192,'backend':'numpy','backend_label':'NumPy'},
              'repair_source':{'path':str(restore/'reservoir_service.py'),'sha256':p.sha(restore/'reservoir_service.py')},
              'restore_source_manifest':{'root':str(restore),'files':{name:p.sha(restore/name) for name in p.RESTORE_NAMES}},
              'restore_runtime_manifest':{'python':str(venv/'bin/python'),'interpreter_files':{str(interpreter):p.sha(interpreter)},
               'pyvenv_cfg':{'path':str(venv/'pyvenv.cfg'),'sha256':p.sha(venv/'pyvenv.cfg')},
               'packages':{'numpy':{'version':'1','files':{str(venv/name):p.sha(venv/name) for name in ('METADATA','RECORD')}}}}}
        def reference(ref):
            if ref['path']=='/fixture':return {'python_executables':{str(holder):p.sha(holder)}}
            if ref['path']=='/resolution':return {'schema':'original_provider_model_resolution_review_v1','accepted':True,
                'snapshot':str(model),'original_argv':spec['plan']['roles']['provider']['identity']['argv'],
                'restart_policy':'pinned','reviewer':'reviewer','evidence':[{}]}
            return manifest
        env=types.SimpleNamespace(validate_manifest=Mock(return_value={'manifest':manifest,'model':str(model)}),
                                  interpreter_paths=Mock(return_value={interpreter}))
        for name,value in [('WORKSPACE',workspace),('STATE_ROOT',workspace/'state')]:
            handle=patch.object(p,name,value);handle.start();self.addCleanup(handle.stop)
        for name,value in [('load_hold',Mock(return_value=env)),('reference',Mock(side_effect=reference)),
                           ('qualified_fixture',Mock(return_value={'python_executables':{str(holder):p.sha(holder)}})),
                           ('validate_provider_restart',Mock())]:
            handle=patch.object(p,name,value);handle.start();self.addCleanup(handle.stop)
        return spec

    def test_complete_manifest_closure_rejects_sparse_groups_and_missing_sources(self):
        spec=self.closure_fixture();p.validate_closure(spec)
        for group in ('provider_packages','model_snapshot','interpreter'):
            changed=copy.deepcopy(spec);changed['input_groups'][group]=changed['input_groups'][group][1:]
            with self.subTest(group=group),self.assertRaises(p.m.Refusal):p.validate_closure(changed)
        for name in ('production_retained.py','transaction_execution.py','recovery_ledger.py','recovery_phases.py','production_recovery_driver.py',
                     'production_fixture_admission.py','production_fixture_history.py','production_entrypoint.py'):
            changed=copy.deepcopy(spec);changed['input_groups']['helpers'].remove(str(p.HERE/name))
            with self.subTest(helper=name),self.assertRaisesRegex(p.m.Refusal,'helper closure'):p.validate_closure(changed)
        changed=copy.deepcopy(spec);del changed['immutable_files'][str(p.WORKSPACE/'persistence.py')]
        with self.assertRaises(p.m.Refusal):p.validate_closure(changed)

    def test_private_restore_rejects_transitive_source_or_runtime_drift(self):
        spec=self.closure_fixture()
        p.validate_restore_inputs(spec,spec['restore_source_manifest'],spec['restore_runtime_manifest'])
        source=copy.deepcopy(spec['restore_source_manifest']);source['files']['persistence.py']='0'*64
        with self.assertRaises(p.m.Refusal):p.validate_restore_inputs(spec,source,spec['restore_runtime_manifest'])
        runtime=copy.deepcopy(spec['restore_runtime_manifest']);runtime['packages']['numpy']['version']='2'
        with self.assertRaises(p.m.Refusal):p.validate_restore_inputs(spec,spec['restore_source_manifest'],runtime)
        runtime=copy.deepcopy(spec['restore_runtime_manifest']);runtime['python']='/different/python'
        with self.assertRaises(p.m.Refusal):p.validate_restore_inputs(spec,spec['restore_source_manifest'],runtime)

    def test_default_host_io_supports_read_only_retained_holder_listener_check(self):
        io=p.ReadOnlySystem();io.command=Mock(return_value=subprocess.CompletedProcess([],1,'',''))
        self.assertEqual(io.all_listeners(123),[])
        io.command.assert_called_once_with('/usr/sbin/lsof','-nP','-a','-p','123','-iTCP','-sTCP:LISTEN','-Fp')
        io.command.return_value=subprocess.CompletedProcess([],1,'','permission error')
        with self.assertRaisesRegex(p.m.Refusal,'inspection failed'):io.all_listeners(123)

    def test_pid_reuse_or_query_error_is_not_process_absence(self):
        io=p.ReadOnlySystem()
        io.command=Mock(side_effect=[subprocess.CompletedProcess([],0,'old',''),
            subprocess.CompletedProcess([],0,'python server',''),subprocess.CompletedProcess([],0,'new','')])
        with self.assertRaisesRegex(p.m.Refusal,'PID changed'):io.process(123)
        io.command=Mock(return_value=subprocess.CompletedProcess([],1,'','ps: permission denied'))
        with self.assertRaisesRegex(p.m.Refusal,'proven absence'):io.process(123)
        io.command=Mock(return_value=subprocess.CompletedProcess([],1,'',''))
        self.assertIsNone(io.process(123))


if __name__ == '__main__': unittest.main(verbosity=2)
