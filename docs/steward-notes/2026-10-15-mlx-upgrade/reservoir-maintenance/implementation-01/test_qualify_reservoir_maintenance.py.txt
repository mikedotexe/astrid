"""Owned fixture guards with stdlib fakes only; never invokes host control."""
import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('qualify_fixture_under_test', HERE / 'qualify_reservoir_maintenance.py')
q = importlib.util.module_from_spec(spec)
spec.loader.exec_module(q)


class FixtureAdapterTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='fake-fixture-adapter-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.token = 'a' * 32
        self.uid = os.getuid()
        # Any accidental non-mocked external command fails this CPU-only suite.
        guarded = patch.object(q, 'command', side_effect=AssertionError('external command forbidden in CPU tests'))
        self.command = guarded.start()
        self.addCleanup(guarded.stop)

    def host(self):
        host = q.FixtureHost.__new__(q.FixtureHost)
        host.root, host.token, host.uid = self.root, self.token, self.uid
        host.executable, host.worker = Path('/fake/python'), self.root / 'worker.py'
        host.ports, host.guards, host.signalled, host.operations = {}, {}, set(), 0
        host.armed = set()
        host.sudo_debug = False
        host.check_owned = Mock()
        host.operation = Mock()
        host.env = types.SimpleNamespace(command_matches=lambda a, b: a == b,
                                         assert_hold=Mock(), process_start=lambda pid: 'start')
        return host

    def identity(self, host, role='provider'):
        return {'pid': 123, 'started_at': 'start', 'argv': host.argv(role)}

    def test_target_allowlist_is_exact_uid_transaction_and_role(self):
        target = q.fixture_target(self.uid, self.token, 'provider')
        q.validate_target(target, self.uid, self.token, 'provider')
        for value in ('gui/501/com.reservoir.service', target + '.extra',
                      q.fixture_target(self.uid, 'b' * 32, 'provider'),
                      q.fixture_target(self.uid, self.token, 'reservoir')):
            with self.subTest(value=value), self.assertRaises(q.m.Refusal):
                q.validate_target(value, self.uid, self.token, 'provider')

    def test_privilege_is_only_debug_and_requires_terminal(self):
        target = q.fixture_target(self.uid, self.token, 'provider')
        with patch.object(q.sys.stdin, 'isatty', return_value=True):
            command = q.debug_command(target, self.uid, self.token, 'provider', ['--program', '/fake/python'], sudo_debug=True)
        self.assertEqual(command[:5], ['/usr/bin/sudo', '--', '/bin/launchctl', 'debug', target])
        with patch.object(q.sys.stdin, 'isatty', return_value=False), self.assertRaises(q.m.Refusal):
            q.debug_command(target, self.uid, self.token, 'provider', [], sudo_debug=True)
        with patch.object(q.os, 'geteuid', return_value=0), self.assertRaises(q.m.Refusal):
            q.debug_command(target, self.uid, self.token, 'provider', [], sudo_debug=False)

    def test_review_binds_current_sources_and_successful_cpu_receipt(self):
        helper = self.root / 'helper.py'
        helper.write_text('original fake helper\n')
        artifacts = {str(helper): q.sha(helper)}
        cpu = self.root / 'cpu.json'
        cpu.write_text(json.dumps({'passed': True, 'artifacts': artifacts}))
        review = self.root / 'review.json'
        review.write_text(json.dumps({'schema': 'reservoir_maintenance_fixture_review_v1', 'approved': True,
                                    'artifacts': artifacts, 'cpu_receipt': {'path': str(cpu), 'sha256': q.sha(cpu)}}))
        with patch.object(q, 'required_artifacts', return_value=[helper]):
            self.assertEqual(q.verify_review(review), artifacts)
            helper.write_text('changed\n')
            with self.assertRaises(q.m.Refusal):
                q.verify_review(review)

    def test_ready_listener_alone_does_not_prove_restored_or_idle(self):
        host = self.host()
        identity = self.identity(host)
        host.inspect_role = Mock(return_value={'process': identity, 'listener_pids': [123], 'held': None})
        host.events = Mock(return_value=[{'phase': 'started'}])
        self.assertIsNone(host._ready_view('provider'))
        host.events.return_value.append({'phase': 'idle'})
        self.assertIsNotNone(host._ready_view('provider'))

    def test_missing_and_incomplete_event_tail_is_not_readiness(self):
        host = self.host()
        self.assertEqual(host.events('provider', 123), [])
        path = self.root / 'provider.events.jsonl'
        started = {'role': 'provider', 'token': self.token, 'pid': 123, 'phase': 'started'}
        path.write_text(json.dumps(started) + '\n' + '{"phase":"idle"')
        self.assertEqual(host.events('provider', 123), [started])

    def test_changed_bootstrap_plist_is_refused_before_any_command(self):
        host = self.host()
        host.inspect_role = Mock(return_value={'job_loaded': False})
        settings = host.plist_settings('provider')
        settings['ProgramArguments'] = ['/foreign/program']
        (self.root / 'provider.plist').write_bytes(q.plistlib.dumps(settings))
        with self.assertRaises(q.m.Refusal):
            host.bootstrap_original('provider', {})
        self.command.assert_not_called()

    def test_inspection_allows_listener_closed_during_cooperative_drain(self):
        host = self.host()
        identity = self.identity(host, 'reservoir')
        host.job = Mock(return_value=(True, 123))
        host.process = Mock(return_value=identity)
        host.listener_ports = Mock(return_value=[])
        host.signalled.add(('reservoir', 123, 'start'))
        (self.root / 'reservoir.ready.json').write_text(json.dumps({'pid': 123, 'token': self.token,
            'role': 'reservoir', 'port': 55555, 'protocol': 'fixture_sentinel_only'}))
        view = host.inspect_role('reservoir')
        self.assertEqual(view['process'], identity)
        self.assertEqual(view['listener_pids'], [])

    def test_inspection_tolerates_ready_file_publication_gap(self):
        host = self.host()
        host.job = Mock(return_value=(True, 123))
        host.process = Mock(return_value=self.identity(host))
        host.listener_ports = Mock(return_value=[55555])
        self.assertEqual(host.inspect_role('provider')['listener_pids'], [])

    def test_foreign_process_command_blocks(self):
        host = self.host()
        host.job = Mock(return_value=(True, 123))
        host.process = Mock(return_value={'pid': 123, 'started_at': 'start', 'argv': ['/foreign/worker']})
        host.listener_ports = Mock(return_value=[])
        with self.assertRaises(q.m.Refusal):
            host.inspect_role('provider')

    def test_bootout_refuses_a_stateful_worker_before_any_command(self):
        host = self.host()
        expected = self.identity(host)
        host.inspect_role = Mock(return_value={'process': expected, 'held': None, 'listener_pids': [123]})
        with self.assertRaises(q.m.Refusal):
            host.remove_held('provider', expected, {})
        self.command.assert_not_called()

    def test_arm_uses_script_first_tail_and_records_intent_before_command(self):
        host = self.host()
        expected = self.identity(host)
        host.inspect_role = Mock(return_value={'process': expected, 'held': None})
        guard = {'transaction_id': self.token}
        self.command.side_effect = None
        self.command.return_value = subprocess.CompletedProcess([], 0, '', '')
        result = host.arm_next_spawn('provider', guard, expected)
        args = self.command.call_args.args
        self.assertEqual(args[:3], ('/bin/launchctl', 'debug', host.target('provider')))
        self.assertEqual(args[3:7], ('--program', '/fake/python', '--', str(self.root / 'model_environment.py')))
        self.assertTrue(result['armed'])
        self.assertEqual(host.operation.call_args_list[0].args[0], 'debug_intent')

    def test_arm_failure_does_not_signal_or_remove_hold(self):
        host = self.host()
        expected = self.identity(host)
        host.inspect_role = Mock(return_value={'process': expected, 'held': None})
        self.command.side_effect = None
        self.command.return_value = subprocess.CompletedProcess([], 1, '', 'not supported')
        with patch.object(q.os, 'kill') as kill, self.assertRaises(q.m.Refusal):
            host.arm_next_spawn('provider', {'transaction_id': self.token}, expected)
        kill.assert_not_called()

    def test_signal_targets_exact_pid_and_is_never_repeated(self):
        host = self.host()
        expected = self.identity(host)
        host.inspect_role = Mock(return_value={'process': expected, 'held': None})
        host.armed.add(('provider', 123, 'start'))
        with patch.object(q.os, 'kill') as kill:
            receipt = host.signal_stop('provider', expected)
            self.assertFalse(receipt['force_used'])
            kill.assert_called_once_with(123, q.signal.SIGTERM)
            with self.assertRaises(q.m.Refusal):
                host.signal_stop('provider', expected)
            kill.assert_called_once()

    def test_signal_requires_a_successfully_armed_override(self):
        host = self.host()
        expected = self.identity(host)
        host.inspect_role = Mock(return_value={'process': expected, 'held': None})
        with patch.object(q.os, 'kill') as kill, self.assertRaises(q.m.Refusal):
            host.signal_stop('provider', expected)
        kill.assert_not_called()

    def test_new_fixture_failure_retains_private_artifacts_without_cleanup(self):
        evidence = self.root / 'evidence'
        output = evidence / 'run.json'
        artifacts = {str(path): q.sha(path) for path in q.required_artifacts()}
        with patch.object(q, 'EVIDENCE_ROOT', evidence), patch.object(q, 'verify_review', return_value=artifacts), \
             patch.object(q, 'FixtureHost', side_effect=RuntimeError('fake setup failure')):
            # The review path is only hashed after the patched review check.
            review = self.root / 'review.json'
            review.write_text('{}')
            result = q.run_fixture(output, review)
        self.assertFalse(result['passed'])
        self.assertTrue(Path(result['root']).is_dir())
        self.assertIn('none; retain', result['failure']['automatic_cleanup'])
        self.assertTrue(output.is_file())
        self.command.assert_not_called()


if __name__ == '__main__':
    unittest.main(verbosity=2)
