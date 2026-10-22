"""CPU-only lifecycle boundary tests for the deliberately blocked Host."""
from pathlib import Path
import plistlib
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

import production_host as h


class ProductionHostTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix='production-host-fake-')
        self.addCleanup(tmp.cleanup); self.root = Path(tmp.name).resolve()
        self.ident = {'pid': 123, 'started_at': 'start', 'argv': ['python', 'server.py']}
        self.plan = {'transaction_id': 'a'*32, 'actor': 'owner', 'host_binding': {'uid': 501},
                     'roles': {role: {'identity': self.ident} for role in h.m.ROLES}}
        self.host = h.ProductionHost.__new__(h.ProductionHost)
        self.host.root = self.root; self.host.plan = self.plan; self.host.io = Mock()
        self.host.guards = {}; self.host.armed = set(); self.host.signalled = set()
        self.host.receipts = {}; self.host.signal_times = {}; self.host.event = Mock()
        self.host.inspect_role = Mock(return_value={'process': self.ident, 'held': None, 'listener_pids': [123]})
        for name in ('kill',):
            mock = patch.object(h.os, name, side_effect=AssertionError('live operation forbidden'))
            self.kill = mock.start(); self.addCleanup(mock.stop)
        mock = patch.object(h.subprocess, 'run', side_effect=AssertionError('live command forbidden'))
        self.command = mock.start(); self.addCleanup(mock.stop)
        self.host._native_command = Mock(side_effect=lambda kind,role,argv,**kwargs:
            self.command(argv) if kind=='debug' else self.host.io.command(*argv))

    def test_every_mutation_and_witness_remains_blocked_before_actions(self):
        for name, args in [('create_hold', ('provider', {})), ('arm_next_spawn', ('provider', {}, self.ident)),
                           ('signal_stop', ('provider', self.ident)), ('remove_held', ('provider', self.ident, {})),
                           ('install_repair', ({},)), ('apply_provider_restart', ({},)), ('bootstrap_original', ('provider', {})),
                           ('release_hold', ('provider', {})), ('collect_witness', ('backup', {}))]:
            with self.subTest(name=name), self.assertRaisesRegex(h.m.Refusal, 'execution disabled'):
                getattr(self.host, name)(*args)
        self.command.assert_not_called(); self.kill.assert_not_called(); self.host.event.assert_not_called()

    def test_controller_still_rejects_production_scope(self):
        with self.assertRaisesRegex(h.m.Refusal, 'production controller admission disabled'):
            h.m.check_target(self.host, 'provider', {}, self.plan)

    def test_reservoir_signal_requires_positive_provider_checkin_and_absence(self):
        self.host._allow = Mock(); self.host._absent = Mock()
        self.host.armed.add(('reservoir', 123, 'start'))
        with self.assertRaisesRegex(h.m.Refusal, 'check-in'):
            self.host.signal_stop('reservoir', self.ident)
        self.kill.assert_not_called()
        self.host.receipts['provider_drain'] = {'passed': True}
        self.kill.side_effect = None
        self.host.signal_stop('reservoir', self.ident)
        self.host._absent.assert_called_with('provider')
        self.kill.assert_called_once_with(123, h.signal.SIGTERM)
        intent = next(call.kwargs for call in self.host.event.call_args_list if call.args == ('signal_intent',))
        self.assertEqual(intent['wall_time_ns'], self.host.signal_times['reservoir'])
        self.assertIsInstance(intent['wall_time_ns'], int)
        with self.assertRaises(h.m.Refusal): self.host.signal_stop('reservoir', self.ident)
        self.kill.assert_called_once()

    def test_debug_requires_terminal_exact_worker_and_records_uncertainty(self):
        self.host._allow = Mock()
        self.host.env = types.SimpleNamespace(assert_hold=Mock())
        with patch.object(h.sys.stdin, 'isatty', return_value=False), self.assertRaises(h.m.Refusal):
            self.host.arm_next_spawn('provider', {}, self.ident)
        self.command.assert_not_called()
        self.command.side_effect = KeyboardInterrupt
        with patch.object(h.sys.stdin, 'isatty', return_value=True), self.assertRaises(KeyboardInterrupt):
            self.host.arm_next_spawn('provider', {}, self.ident)
        self.assertIn('provider', self.host.armed)
        self.assertNotIn(('provider', 123, 'start'), self.host.armed)
        with patch.object(h.sys.stdin, 'isatty', return_value=True), self.assertRaises(h.m.Refusal):
            self.host.arm_next_spawn('provider', {}, self.ident)
        self.command.assert_called_once()
        self.kill.assert_not_called()

    def test_live_worker_can_never_be_booted_out(self):
        self.host._allow = Mock()
        with self.assertRaisesRegex(h.m.Refusal, 'owned listener-free holder'):
            self.host.remove_held('provider', self.ident, {})
        self.host.io.command.assert_not_called()

    def test_same_named_holder_does_not_hide_surviving_old_worker(self):
        self.host._allow = Mock()
        holder = dict(self.ident, pid=456)
        self.host.inspect_role.return_value = {'process': holder, 'held': {'guard': {}}, 'listener_pids': []}
        self.host.io.process.return_value = self.ident
        with self.assertRaisesRegex(h.m.Refusal, 'survives beside holder'):
            self.host.remove_held('provider', holder, {})
        self.host.io.command.assert_not_called()

    def test_startup_accepts_complete_per_handle_semantics_not_count(self):
        expected = {'astrid': {'entity': 'astrid', 'backend': 'numpy', 'tick_count': 11, 'mode': 'hold'},
                    'historic': {'entity': 'saved', 'backend': 'numpy', 'tick_count': 7, 'mode': 'quiet'}}
        config = {'input_dim': 32, 'n_nodes': 192, 'backend_label': 'NumPy'}
        lines = ["restored 'astrid' (entity=astrid, backend=numpy, ticks=11, mode=hold)",
                 "restored 'historic' (entity=saved, backend=numpy, ticks=7, mode=quiet)",
                 'listening on 127.0.0.1:7881 (input_dim=32, n_nodes=192, backend=NumPy)']
        data = ('\n'.join(lines)+'\n').encode()
        self.assertEqual(h.startup_log(data, expected, config)['restored'], expected)
        for broken in [data.replace((lines[1]+'\n').encode(), b''), data + (lines[0]+'\n').encode(),
                       data.replace(b'ticks=11', b'ticks=12'), data+b'thermostat restore failed for astrid\n',
                       b'restored 2 handles from disk: astrid, historic\n'+(lines[2]+'\n').encode()]:
            with self.subTest(broken=broken), self.assertRaises(h.m.Refusal): h.startup_log(broken, expected, config)

    def test_every_final_active_handle_needs_fresh_snapshot_and_sidecar(self):
        entries = {'astrid.npz': {'kind': 'file', 'mtime_ns': 200},
                   'astrid_thermostats.json': {'kind': 'file', 'mtime_ns': 200},
                   'historic.npz': {'kind': 'file', 'mtime_ns': 1}}
        h.verify_final_files(entries, ['astrid'], 100)
        for name in ('astrid.npz', 'astrid_thermostats.json'):
            missing = {key: value for key, value in entries.items() if key != name}
            with self.subTest(name=name), self.assertRaises(h.m.Refusal):
                h.verify_final_files(missing, ['astrid'], 100)
            stale = dict(entries, **{name: {'kind': 'file', 'mtime_ns': 99}})
            with self.assertRaises(h.m.Refusal): h.verify_final_files(stale, ['astrid'], 100)
        with self.assertRaises(h.m.Refusal): h.verify_final_files(entries, ['missing'], 100)

    @patch('production_retained.ready_identity')
    def test_provider_pin_is_atomic_and_bootstrap_failure_preserves_exact_evidence(self, ready):
        self.host.startups={'reservoir':{'identity':self.ident}}
        original = self.root / 'original.plist'
        installed = self.root / 'installed.plist'
        restart = self.root / 'restart.plist'
        journal = self.root / 'journal.json'
        snapshot = '/private/checkpoint/snapshot'
        launch = ['/env/bin/python', '/source/provider.py', '--model', 'hub/model', '--port', '8090']
        old_argv = ['/framework/Python', *launch[1:]]
        self.ident = {'pid': 123, 'started_at': 'old', 'argv': old_argv}
        self.host.plan['roles']['provider']['identity'] = self.ident
        old_bytes = plistlib.dumps({'Label': 'example', 'ProgramArguments': launch, 'KeepAlive': True})
        pinned_bytes = plistlib.dumps({'Label': 'example', 'ProgramArguments': h.p.pinned_argv(launch, snapshot), 'KeepAlive': True})
        original.write_bytes(old_bytes); installed.write_bytes(old_bytes); restart.write_bytes(pinned_bytes)
        journal.write_bytes(b'{}\n')
        self.host.spec = {'plan': self.host.plan, 'originals': {'provider': {'plist': str(original)}},
                          'provider_restart_argv': h.p.pinned_argv(old_argv, snapshot),
                          'provider_restart_plist': {'path': str(restart), 'sha256': h.p.sha(restart)},
                          'original_provider_manifest': {}}
        self.host.env = types.SimpleNamespace(option=lambda argv, key: argv[argv.index(key) + 1])
        self.host.bound_stats = {}; self.host.provider_pinned = False
        self.host._allow = Mock(); self.host._absent = Mock()
        self.host.receipts = {'startup': {'passed': True}, 'provider_drain': {'evidence': {
            'journal': {'sha256': h.p.sha(journal)}}}}
        self.host.io.command.return_value = types.SimpleNamespace(returncode=1,native_command={'path':'/inert/exit','sha256':'a'*64})
        with patch.dict(h.p.PLISTS, provider=installed), patch.object(h.p, 'JOURNAL', journal), \
             patch.object(h.p, 'reference', return_value={'argv': ['python', '--model', snapshot]}), \
             patch.object(h.p, 'cursor', return_value={'offset': 0}), \
             patch.object(h.os, 'replace', side_effect=OSError('atomic replacement refused')):
            with self.assertRaisesRegex(OSError, 'atomic replacement refused'):
                self.host.apply_provider_restart({})
        self.assertEqual(installed.read_bytes(), old_bytes)
        self.assertEqual(original.read_bytes(), old_bytes)
        self.host.io.command.assert_not_called()
        self.assertFalse(self.host.provider_pinned)
        # A distinct transaction is the only tested second attempt. Never replay
        # an unresolved same-transaction temporary file or operation sequence.
        self.host.plan['transaction_id'] = 'b' * 32
        self.host.event.reset_mock()
        with patch.dict(h.p.PLISTS, provider=installed), patch.object(h.p, 'JOURNAL', journal), \
             patch.object(h.p, 'reference', return_value={'argv': ['python', '--model', snapshot]}), \
             patch.object(h.p, 'cursor', return_value={'offset': 0}):
            result = self.host.apply_provider_restart({})
            self.assertEqual(result['provider_settings_sha256'], h.p.sha(restart))
            self.assertEqual(result['provider_restart_argv'], self.host.spec['provider_restart_argv'])
            with self.assertRaisesRegex(h.m.Refusal, 'already applied'):
                self.host.apply_provider_restart({})
            with self.assertRaisesRegex(h.m.Refusal, 'bootstrap failed'):
                self.host.bootstrap_original('provider', {})
        self.assertEqual(installed.read_bytes(), pinned_bytes)
        self.assertEqual(original.read_bytes(), old_bytes)
        self.assertTrue(self.host.provider_pinned)
        intent = next(call.kwargs for call in self.host.event.call_args_list if call.args == ('bootstrap_intent',))
        self.assertEqual(intent['applied_plist_sha256'], h.p.sha(restart))
        self.assertEqual(intent['original_rollback_plist_sha256'], h.p.sha(original))
        returned=next(call.kwargs for call in self.host.event.call_args_list if call.args==('bootstrap_command_returned',))
        self.assertEqual(returned['returncode'],1)
        self.assertGreaterEqual(returned['wall_time_ns'],intent['wall_time_ns'])
        self.host.io.command.assert_called_once()

    def test_failed_restore_or_arbitrary_callback_cannot_install(self):
        self.host._allow = Mock(); self.host._absent = Mock()
        self.host.restore_runner = Mock()
        with self.assertRaisesRegex(h.m.Refusal, 'exact reviewed finite restore runner'):
            self.host.collect_witness('private_restore', {})
        self.host.restore_runner.assert_not_called()
        self.host.receipts = {'private_restore': {'passed': False}}
        with self.assertRaisesRegex(h.m.Refusal, 'real restore acceptance required'):
            self.host.install_repair({})
        self.host.event.assert_not_called()

    def test_bootstrap_requires_distinct_process_identity_with_exact_arguments(self):
        self.host.plan['timeout_s'] = 1
        retained = self.root / 'original.plist'; installed = self.root / 'installed.plist'
        retained.write_bytes(b'same original settings'); installed.write_bytes(retained.read_bytes())
        self.host.spec = {'originals': {'reservoir': {'plist': str(retained)}}}
        self.host.receipts = {'backup': {'evidence': {'source_inventory': {}}}}
        self.host.startups = {}; self.host._allow = Mock(); self.host._absent = Mock()
        self.host.io.command.return_value = types.SimpleNamespace(returncode=0,native_command={'path':'/inert/exit','sha256':'a'*64})
        state = types.SimpleNamespace(inventory_tree=lambda path: {})
        fresh = dict(self.ident, pid=456, started_at='new start')
        wrong = dict(fresh, argv=['python', 'foreign.py'])
        with patch.dict(h.sys.modules, reservoir_maintenance_state=state), \
             patch.dict(h.p.PLISTS, reservoir=installed), patch.object(h.p, 'cursor', return_value={'offset': 0}):
            for identity in (self.ident, wrong):
                self.host.inspect_role.return_value = {'process': identity, 'listener_pids': [identity['pid']]}
                with self.subTest(identity=identity), self.assertRaisesRegex(h.m.Refusal, 'command or identity differs'):
                    self.host.bootstrap_original('reservoir', {})
                self.assertEqual(self.host.event.call_args.args,('bootstrap_command_returned',))
            self.host.inspect_role.return_value = {'process': fresh, 'listener_pids': [fresh['pid']]}
            self.assertEqual(self.host.bootstrap_original('reservoir', {})['identity'], fresh)
            self.assertEqual([row.args[0] for row in self.host.event.call_args_list[-3:]],
                             ['bootstrap_intent','bootstrap_command_returned','bootstrap_ready'])


    def test_release_requires_current_readiness_even_with_prior_passed_receipt(self):
        self.host._allow=Mock();self.host.env=Mock()
        self.host.receipts={'provider_ready':{'passed':True}}
        self.host.startups={role:{'identity':self.ident} for role in h.m.ROLES}
        with patch('production_retained.ready_identity',side_effect=h.m.Refusal('current health failed')):
            with self.assertRaisesRegex(h.m.Refusal,'current health'):
                self.host.release_hold('provider',{})
        self.host.env.release_hold.assert_not_called();self.host.event.assert_not_called()


if __name__ == '__main__': unittest.main(verbosity=2)
