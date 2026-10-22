"""Recovery failure boundaries with fake observations only; no host operations."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import recover_owned_fixture as r
q, m = r.q, r.m


class RecoveryTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix='owned-recovery-fake-')
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()
        for obj, name, value in [(r, 'ROOT', self.root)]:
            mock = patch.object(obj, name, value)
            mock.start(); self.addCleanup(mock.stop)
        for obj, name in [(q, 'command'), (r.subprocess, 'run'), (q.os, 'kill')]:
            mock = patch.object(obj, name, side_effect=AssertionError('host operation forbidden in CPU test'))
            item = mock.start(); self.addCleanup(mock.stop)
            if name == 'run': self.run_command = item
        self.target = {'provider_holder': {'pid': 77047, 'started_at': 'holder-start'},
            'reservoir_worker': {'pid': 76424, 'started_at': 'worker-start', 'argv': ['exact-worker']},
            'second_request': 'second', 'expected_final_state': {'fixture_astrid': {'ticks': 22}, 'fixture_minime': {'ticks': 7}}}

    def host(self):
        h = Mock()
        h.control.side_effect = lambda role: self.root / ('hold-' + role)
        h.hold_argv.return_value = ['exact-holder']
        h.env.command_matches.side_effect = lambda a, b: a == b
        h.env.process_start.return_value = None
        h.env.assert_hold = Mock()
        h.listener_ports.return_value = []
        h.guards = {}
        h.armed = set()
        h.target.return_value = 'exact-target'
        return h

    def holder_view(self):
        identity = dict(self.target['provider_holder'], argv=['exact-holder'])
        return {'job_loaded': True, 'process': identity, 'listener_pids': [],
                'held': {'guard': r.GUARD, 'identity': identity}}

    def test_only_exact_provider_holder_is_removed(self):
        host = self.host(); view = self.holder_view()
        host.inspect_role.return_value = view
        r.remove_provider_holder(host, self.target)
        host.remove_held.assert_called_once_with('provider', view['process'], r.GUARD)
        host.signal_stop.assert_not_called()
        host.arm_next_spawn.assert_not_called()

    def test_wrong_holder_pid_start_guard_or_argv_is_refused(self):
        for field, value in [('pid', 999), ('started_at', 'other'), ('argv', ['foreign'])]:
            host = self.host(); view = self.holder_view()
            view['held']['identity'][field] = value
            host.inspect_role.return_value = view
            with self.subTest(field=field), self.assertRaises(m.Refusal):
                r.remove_provider_holder(host, self.target)
            host.remove_held.assert_not_called()
        host = self.host(); view = self.holder_view()
        view['held']['guard'] = dict(r.GUARD, actor='foreign')
        host.inspect_role.return_value = view
        with self.assertRaises(m.Refusal): r.remove_provider_holder(host, self.target)
        host.remove_held.assert_not_called()

    def test_worker_or_absent_provider_without_prior_removal_is_refused(self):
        for view in [{'job_loaded': True, 'held': None}, {'job_loaded': False}]:
            host = self.host(); host.inspect_role.return_value = view
            with patch.object(r, 'operation_records', return_value=[{}] * 27), self.assertRaises(m.Refusal):
                r.remove_provider_holder(host, self.target)
            host.remove_held.assert_not_called()

    def test_absent_provider_continuation_checks_actual_old_pid_and_listener(self):
        host = self.host(); host.inspect_role.return_value = {'job_loaded': False}
        rows = [{}] * 27 + [{'operation': 'held_bootout_intent', 'role': 'provider',
                             'identity': dict(self.target['provider_holder'], argv=['exact-holder'])}]
        with patch.object(r, 'operation_records', return_value=rows):
            r.remove_provider_holder(host, self.target)
            host.env.process_start.return_value = 'still survives'
            with self.assertRaises(m.Refusal): r.remove_provider_holder(host, self.target)
        host.remove_held.assert_not_called()

    def debug_host(self):
        h = r.RecoveryHost.__new__(r.RecoveryHost)
        h.target_record = self.target; h.interactive_debug = False
        h.uid = q.os.getuid(); h.token = r.TOKEN; h.root = self.root
        h.executable = r.PYTHON; h.check_owned = Mock()
        h.env = types.SimpleNamespace(assert_hold=Mock())
        h.armed = set(); h.operation = Mock()
        h.inspect_role = Mock(return_value={'process': self.target['reservoir_worker'], 'held': None})
        return h

    def test_cached_auth_is_scoped_to_exact_reservoir_debug(self):
        h = self.debug_host()
        self.run_command.side_effect = None
        self.run_command.return_value = subprocess.CompletedProcess([], 0)
        h.arm_next_spawn('reservoir', r.GUARD, self.target['reservoir_worker'])
        argv = self.run_command.call_args.args[0]
        self.assertEqual(argv[:5], ['/usr/bin/sudo', '-n', '--', '/bin/launchctl', 'debug'])
        self.assertEqual(argv[5], q.fixture_target(h.uid, r.TOKEN, 'reservoir'))
        self.assertEqual(argv[9], str(self.root / 'model_environment.py'))
        self.assertEqual(self.run_command.call_args.kwargs, {'check': False, 'timeout': 180, 'capture_output': True, 'text': True})
        self.assertIn(('reservoir', 76424, 'worker-start'), h.armed)

    def test_cached_auth_failure_retains_guard_without_arming(self):
        h = self.debug_host()
        self.run_command.side_effect = None
        self.run_command.return_value = subprocess.CompletedProcess([], 1, '', 'sudo: a password is required\n')
        with self.assertRaisesRegex(m.Refusal, 'worker not signalled'):
            h.arm_next_spawn('reservoir', r.GUARD, self.target['reservoir_worker'])
        self.assertEqual(h.armed, set())
        self.assertEqual([c.args[0] for c in h.operation.call_args_list], ['debug_intent', 'debug_result'])

    def test_interrupted_debug_has_durable_intent_and_never_arms(self):
        h = self.debug_host()
        self.run_command.side_effect = KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):
            h.arm_next_spawn('reservoir', r.GUARD, self.target['reservoir_worker'])
        self.assertEqual(h.armed, set())
        self.assertEqual([c.args[0] for c in h.operation.call_args_list], ['debug_intent'])

    def test_interactive_debug_requires_terminal_and_wrong_role_is_refused(self):
        h = self.debug_host(); h.interactive_debug = True
        with patch.object(r.sys.stdin, 'isatty', return_value=False), self.assertRaises(m.Refusal):
            h.arm_next_spawn('reservoir', r.GUARD, self.target['reservoir_worker'])
        with self.assertRaises(m.Refusal): h.arm_next_spawn('provider', r.GUARD, self.target['reservoir_worker'])
        self.run_command.assert_not_called()

    def snapshot(self, host):
        final = self.target['expected_final_state']
        host.events.return_value = [{'phase': 'stop_received', 'monotonic': 0},
            {'phase': 'final_snapshot_started', 'handles': sorted(final)},
            *[{'phase': 'handle_saved', 'name': n, 'value': v} for n, v in final.items()],
            {'phase': 'final_snapshot_complete', 'state': final}, {'phase': 'drained', 'monotonic': 6.1}]
        (self.root / 'state.json').write_text(json.dumps(final))

    def test_snapshot_requires_both_handles_six_seconds_and_durable_state(self):
        host = self.host(); self.snapshot(host)
        r.verify_final_snapshot(host, self.target)
        host.events.return_value[-1]['monotonic'] = 4
        with self.assertRaises(m.Refusal): r.verify_final_snapshot(host, self.target)
        self.snapshot(host); host.events.return_value.pop(2)
        with self.assertRaises(m.Refusal): r.verify_final_snapshot(host, self.target)
        self.snapshot(host); (self.root / 'state.json').write_text('{}')
        with self.assertRaises(m.Refusal): r.verify_final_snapshot(host, self.target)

    def test_existing_signal_intent_waits_without_repeating_signal_or_debug(self):
        host = self.host(); self.snapshot(host)
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
        held = {'job_loaded': True, 'held': {'guard': r.GUARD, 'identity': {'pid': 90000}}}
        rows = [{}] * 27 + [{'operation': 'signal_intent', 'role': 'reservoir', 'identity': self.target['reservoir_worker']}]
        with patch.object(r, 'operation_records', return_value=rows), patch.object(q, 'wait_for', return_value=held):
            r.finish_reservoir(host, self.target)
        host.signal_stop.assert_not_called(); host.arm_next_spawn.assert_not_called()
        host.remove_held.assert_called_once_with('reservoir', {'pid': 90000}, r.GUARD)

    def test_fresh_reservoir_path_orders_hold_arm_signal_wait_before_holder_removal(self):
        host = self.host(); self.snapshot(host)
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
        held = {'job_loaded': True, 'held': {'guard': r.GUARD, 'identity': {'pid': 90000}}}
        with patch.object(r, 'operation_records', return_value=[{}]*27), patch.object(q, 'wait_for', return_value=held):
            r.finish_reservoir(host, self.target)
        names = [call[0] for call in host.mock_calls]
        self.assertLess(names.index('create_hold'), names.index('arm_next_spawn'))
        self.assertLess(names.index('arm_next_spawn'), names.index('signal_stop'))
        self.assertLess(names.index('signal_stop'), names.index('remove_held'))
        host.signal_stop.assert_called_once_with('reservoir', self.target['reservoir_worker'])

    def test_arm_failure_stops_before_signal_wait_or_removal(self):
        host = self.host()
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
        host.arm_next_spawn.side_effect = m.Refusal('auth failed')
        with patch.object(r, 'operation_records', return_value=[{}]*27), self.assertRaises(m.Refusal):
            r.finish_reservoir(host, self.target)
        host.signal_stop.assert_not_called(); host.remove_held.assert_not_called()

    def test_uncertain_signal_times_out_without_repeat_or_forced_cleanup(self):
        host = self.host()
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
        rows = [{}]*27 + [{'operation': 'signal_intent', 'role': 'reservoir', 'identity': self.target['reservoir_worker']}]
        with patch.object(r, 'operation_records', return_value=rows), \
             patch.object(q, 'wait_for', side_effect=m.Refusal('bounded wait expired')), self.assertRaises(m.Refusal):
            r.finish_reservoir(host, self.target)
        host.signal_stop.assert_not_called(); host.remove_held.assert_not_called()

    def debug_rows(self, host, result=None):
        command = ['/usr/bin/sudo', '-n', '--', '/bin/launchctl', 'debug', host.target('reservoir'),
                   '--program', str(r.PYTHON), '--', *host.hold_argv('reservoir')[1:]]
        rows = [{'operation': 'debug_intent', 'role': 'reservoir', 'recovery': True, 'command': command}]
        if result is not None:
            rows.append({'operation': 'debug_result', 'role': 'reservoir', 'recovery': True, 'returncode': result,
                         'authorization_refused_before_command': result != 0, 'stdout': '',
                         'stderr': 'sudo: a password is required' if result else ''})
        return rows

    def test_uncertain_debug_never_rearms_or_signals(self):
        host = self.host()
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
        with patch.object(r, 'operation_records', return_value=[{}]*27 + self.debug_rows(host)), self.assertRaisesRegex(m.Refusal, 'uncertain'):
            r.finish_reservoir(host, self.target)
        host.arm_next_spawn.assert_not_called(); host.signal_stop.assert_not_called()

    def test_known_failed_debug_allows_explicit_new_attempt_but_success_is_not_rearmed(self):
        for code in (1, 0):
            host = self.host(); self.snapshot(host)
            host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': self.target['reservoir_worker']}
            held = {'job_loaded': True, 'held': {'guard': r.GUARD, 'identity': {'pid': 90000}}}
            with self.subTest(code=code), patch.object(r, 'operation_records', return_value=[{}]*27 + self.debug_rows(host, code)), \
                 patch.object(q, 'wait_for', return_value=held):
                r.finish_reservoir(host, self.target)
            self.assertEqual(host.arm_next_spawn.call_count, 1 if code else 0)
            host.signal_stop.assert_called_once()

    def test_pending_signal_publication_gap_waits_without_repeating(self):
        host = self.host(); self.snapshot(host)
        host.inspect_role.return_value = {'job_loaded': True, 'held': None, 'process': None}
        rows = [{}]*27 + [{'operation': 'signal_intent', 'role': 'reservoir', 'identity': self.target['reservoir_worker']}]
        held = {'job_loaded': True, 'held': {'guard': r.GUARD, 'identity': {'pid': 90000}}}
        with patch.object(r, 'operation_records', return_value=rows), patch.object(q, 'wait_for', return_value=held):
            r.finish_reservoir(host, self.target)
        host.signal_stop.assert_not_called(); host.arm_next_spawn.assert_not_called()

    def test_malformed_or_repeated_debug_outcome_is_refused(self):
        host = self.host()
        for rows in [self.debug_rows(host, 0) + self.debug_rows(host), self.debug_rows(host) + self.debug_rows(host),
                     [{'operation': 'debug_result', 'role': 'reservoir', 'returncode': 0}]]:
            with self.subTest(rows=rows), self.assertRaises(m.Refusal): r.debug_state(rows, host)

    def test_unknown_nonzero_debug_result_never_authorizes_retry(self):
        host = self.host()
        for error in ['Could not debug service: 5: Input/output error', '', 'sudo: a password is required\nextra']:
            rows = self.debug_rows(host, 1)
            rows[-1]['stderr'] = error
            self.assertEqual(r.debug_state(rows, host), 'uncertain')
        rows = self.debug_rows(host, 1)
        rows[0]['command'].remove('-n')
        self.assertEqual(r.debug_state(rows, host), 'uncertain')

    def test_non_auth_command_failure_is_recorded_uncertain(self):
        h = self.debug_host()
        self.run_command.side_effect = None
        self.run_command.return_value = subprocess.CompletedProcess([], 5, '', 'launchctl: unknown outcome')
        with self.assertRaisesRegex(m.Refusal, 'uncertain'):
            h.arm_next_spawn('reservoir', r.GUARD, self.target['reservoir_worker'])
        self.assertFalse(h.operation.call_args.kwargs['authorization_refused_before_command'])
        self.assertEqual(h.armed, set())

    def test_unexpected_holder_without_signal_intent_is_not_removed(self):
        host = self.host(); host.inspect_role.return_value = self.holder_view()
        with patch.object(r, 'operation_records', return_value=[{}]*27), self.assertRaises(m.Refusal):
            r.finish_reservoir(host, self.target)
        host.remove_held.assert_not_called()


if __name__ == '__main__':
    unittest.main(verbosity=2)
