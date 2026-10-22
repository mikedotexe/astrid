"""Tiny stdlib/fake tests only; no child, signal, socket or native adapter call."""
from __future__ import annotations

import copy
import contextlib
import io
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

import owned_fixture_coordinator as c
import owned_fixture_injection as injection
import owned_fixture_records as r
import owned_fixture_result as result
import owned_fixture_run as run
import owned_fixture_system as system
import owned_pin_system as pin_system
from owned_fixture_plan import proposed_schedule


def identity(pid):
    return {'pid': pid, 'started_at': 'inert-source-test', 'argv': ['inert-python']}


class Plan(unittest.TestCase):
    def release(self): return {'executor_plan': proposed_schedule()['executor_plan']}

    def test_sealed_71_steps_and_initializer_placement(self):
        release = self.release(); phases = run.phases(release)
        self.assertEqual(len(release['executor_plan']), 71)
        self.assertEqual(len(phases), 73)
        self.assertEqual(phases[0], ('lock_pair', release['executor_plan'][:2]))
        executed = [step for action, step in phases if action.startswith('executor:')]
        self.assertEqual(executed, release['executor_plan'][2:])
        for initializer, kind in (('cleanup_initialize', 'cleanup'), ('pin_initialize', 'pin')):
            index = next(i for i, row in enumerate(phases) if row[0] == initializer)
            self.assertEqual(phases[index + 1][1]['kind'], kind)
        self.assertEqual(phases[-1], ('finalize', None))

    def test_modified_reordered_or_extra_schedule_refuses_before_callback(self):
        original = self.release()
        for variant in ('reordered', 'extra', 'exit', 'injection'):
            release = copy.deepcopy(original); steps = release['executor_plan']
            if variant == 'reordered': steps[3], steps[4] = steps[4], steps[3]
            elif variant == 'extra': steps.append(copy.deepcopy(steps[-1]))
            elif variant == 'exit': steps[2]['expected_exit'] = 0
            else: steps[2]['injection'] = None
            with self.subTest(variant=variant), self.assertRaises(ValueError):
                run.walk(release, lambda *_: self.fail('unsealed plan executed'))

    def test_every_parent_boundary_failure_stops_without_retry_or_cleanup(self):
        release = self.release(); expected = [action for action, _ in run.phases(release)]
        for failed_index in range(len(expected)):
            seen = []
            def perform(action, detail):
                seen.append(action)
                if len(seen) - 1 == failed_index: raise RuntimeError('inert lost boundary')
            with self.subTest(boundary=expected[failed_index]), self.assertRaises(RuntimeError):
                run.walk(release, perform)
            self.assertEqual(seen, expected[:failed_index + 1])

    def test_only_expected_75_and76_entries_are_sealed(self):
        steps = self.release()['executor_plan']
        self.assertEqual([x['kind'] for x in steps if x['expected_exit'] == 76], ['lock_refusal'])
        for i, step in enumerate(steps):
            if step['expected_exit'] == 75:
                self.assertIsNotNone(step['injection'])
                following = steps[i + 1]
                self.assertEqual(following['action_id'], step['action_id'])
                self.assertEqual(following['mode'], 'reconcile')
                self.assertEqual(following['expected_exit'], 0)


class Disabled(unittest.TestCase):
    def test_default_main_has_no_native_or_release_access(self):
        with mock.patch.object(c, 'validate_release', side_effect=AssertionError('unexpected release read')), \
             mock.patch.object(run.subprocess, 'run', side_effect=AssertionError('unexpected process')), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertEqual(run.main([]), 0)
        self.assertIn('not_executed', output.getvalue())

    def test_all_native_entry_boundaries_refuse_before_io(self):
        adapter = system.OwnedSystem.__new__(system.OwnedSystem)
        with mock.patch.object(c, 'FIXTURE_EXECUTION_ENABLED', False), \
             mock.patch.object(run.subprocess, 'run', side_effect=AssertionError('forbidden native IO')):
            with self.assertRaisesRegex(ValueError, 'disabled'): run.execute('/never-read')
            with self.assertRaisesRegex(ValueError, 'disabled'): run.initial_observations({}, Path('/never-read'))
            with self.assertRaisesRegex(ValueError, 'disabled'): adapter._run(['/never-run'])
            with self.assertRaisesRegex(ValueError, 'disabled'): pin_system.PinSystem._run(['/never-run'])

    def test_ps_absence_requires_exact_nonerror_output(self):
        adapter = system.OwnedSystem.__new__(system.OwnedSystem)
        clean = types.SimpleNamespace(returncode=1, stdout='', stderr='')
        with mock.patch.object(adapter, '_run', return_value=clean): self.assertIsNone(adapter._start(101))
        for changed in (types.SimpleNamespace(returncode=2, stdout='', stderr=''),
                        types.SimpleNamespace(returncode=1, stdout='', stderr='query failed'),
                        types.SimpleNamespace(returncode=0, stdout='', stderr='')):
            with mock.patch.object(adapter, '_run', return_value=changed), self.assertRaises(ValueError):
                adapter._start(101)

    def test_pin_raw_timestamp_is_captured_after_inert_query_return(self):
        order = []; argv = ['/bin/launchctl', 'print', 'inert-never-executed-target']
        def query(*args, **kwargs):
            order.append('query-return')
            return types.SimpleNamespace(returncode=113, stdout='', stderr='inert absence')
        def now(): order.append('observed-time'); return 123456789
        with mock.patch.object(c, 'FIXTURE_EXECUTION_ENABLED', True), \
             mock.patch.object(pin_system.subprocess, 'run', side_effect=query) as native, \
             mock.patch.object(pin_system.time, 'time_ns', side_effect=now):
            row = pin_system.PinSystem._run(argv)
        native.assert_called_once_with(argv, check=False, capture_output=True, text=True, timeout=20)
        self.assertEqual(order, ['query-return', 'observed-time'])
        self.assertEqual(row, {'returncode':113, 'stdout':'', 'stderr':'inert absence',
                              'argv':argv, 'observed_wall_time_ns':123456789})


class NativeDispatch(unittest.TestCase):
    """Exercise the real parent ledger/dispatch with only inert Host methods."""
    def fixture(self, step):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve(); parent = identity(100)
        release = {'fixture': {'root': str(root)}, 'executor_plan': proposed_schedule()['executor_plan']}
        release_path = root / 'release.json'; r.write(release_path, release)
        host = types.SimpleNamespace(root=root, owner={'coordinator_identity': parent},
            executor_identity=mock.Mock(return_value=parent), plan={'inert': 'plan'}, spec={'inert': 'spec'},
            recovery_inspection=mock.Mock(return_value={'inert': 'inspection'}))
        operation = run.NativeOperations(release_path, release, {})
        operation.host = mock.Mock(return_value=host)
        # Other tests seal all71 phases. Here the real ledger is kept tiny so the
        # test isolates perform's durable transition and exact one-step dispatch.
        action = 'executor:' + step['id']; operation.actions = ['setup', action]
        ledger = r.Ledger(root, 'parent', {'release_sha256': operation.release_ref['sha256'],
                          'parent_identity_sha256': r.digest(parent)}, create=True)
        claim = ledger.append('claim', 'setup', executor_identity=parent)
        ledger.append('result', 'setup', claim_sha256=r.digest(claim), result={'inert': True})
        return operation, host, action, ledger

    def test_out_of_order_and_attempted_refuse_before_dispatch(self):
        step = next(s for s in proposed_schedule()['executor_plan'] if s['kind'] == 'fresh')
        for attempted in (False, True):
            with self.subTest(attempted=attempted):
                operation, host, action, ledger = self.fixture(step)
                if attempted: ledger.append('claim', action, executor_identity=host.executor_identity())
                before = copy.deepcopy(ledger.rows)
                with mock.patch.object(c, 'prepare_executor') as prepare, \
                     mock.patch.object(c, 'launch_executor') as launch, \
                     mock.patch.object(operation, 'expected_heads') as heads, \
                     self.assertRaisesRegex(ValueError, 'never replay'):
                    operation.perform(action if attempted else 'executor:out-of-order', step)
                prepare.assert_not_called(); launch.assert_not_called(); heads.assert_not_called()
                self.assertEqual(ledger.reload(), before)

    def test_dispatch_error_retains_failed_claim_and_blocks_retry_or_next_action(self):
        step = next(s for s in proposed_schedule()['executor_plan'] if s['kind'] == 'fresh')
        operation, host, action, ledger = self.fixture(step)
        operation.actions.append('executor:next-never-run')
        with mock.patch.object(c, 'prepare_executor', side_effect=RuntimeError('inert dispatch error')) as prepare, \
             mock.patch.object(c, 'launch_executor') as launch:
            with self.assertRaisesRegex(RuntimeError, 'inert dispatch error'):
                operation.perform(action, step)
            retained = ledger.reload(); claim, failed = retained[-2:]
            self.assertEqual([claim['type'], failed['type']], ['claim', 'failed'])
            self.assertEqual(claim['executor_identity'], host.executor_identity())
            self.assertEqual(failed['claim_sha256'], r.digest(claim))
            self.assertIs(failed['retry_permitted'], False); self.assertIs(failed['force_used'], False)
            self.assertNotIn(action, ledger.completed())
            for selected in (action, 'executor:next-never-run'):
                with self.assertRaisesRegex(ValueError, 'never replay'): operation.perform(selected, step)
            self.assertEqual(ledger.reload(), retained)
            prepare.assert_called_once(); launch.assert_not_called()

    def test_resume_cleanup_and_pin_dispatch_bind_actual_expected_head(self):
        import owned_fixture_cleanup as cleanup
        for kind in ('resume_one', 'cleanup', 'pin'):
            with self.subTest(kind=kind):
                step = next(s for s in proposed_schedule()['executor_plan'] if s['kind'] == kind)
                operation, host, action, ledger = self.fixture(step)
                checkpoint = {'action_id': step['action_id'], 'inert': 'checkpoint'}
                core_journal = object(); core_ledger = {'inert': 'recovery-ledger'}
                operation.core = {
                    'm': types.SimpleNamespace(Journal=mock.Mock(return_value=core_journal)),
                    'recovery_ledger': types.SimpleNamespace(read_ledger=mock.Mock(return_value=core_ledger)),
                    'recovery_phases': types.SimpleNamespace(checkpoint_from_inspection=mock.Mock(return_value=checkpoint)),
                    'production_recovery_driver': types.SimpleNamespace(expected_heads=mock.Mock(return_value={'main': 'exact-main-head'}))}
                if kind == 'pin':
                    binding = {'inert': 'pin'}; pin = r.Ledger(host.root, 'pin', binding, create=True)
                    r.write(host.root / 'pin/inputs.json', {'binding': binding}); expected = {'pin': pin.head}
                else: expected = {'cleanup': 'exact-cleanup-head'} if kind == 'cleanup' else {'main': 'exact-main-head'}
                directory = host.root / 'executor'; directory.mkdir(mode=0o700)
                request_ref = r.write(directory / 'request.json', {'expected_heads': expected})
                exit_ref = r.write(directory / 'exit.json', {'returncode': step['expected_exit']})
                def prepare(root, release_ref, step_id, selected_host, *, expected_heads):
                    self.assertEqual((root, release_ref, step_id, selected_host),
                                     (host.root, operation.release_ref, step['id'], host))
                    self.assertEqual(expected_heads, expected)
                    self.assertEqual(ledger.reload()[-1]['type'], 'claim')
                    self.assertEqual(ledger.rows[-1]['action'], action)
                    return directory
                with mock.patch.object(cleanup, 'attach', return_value=(None, types.SimpleNamespace(head='exact-cleanup-head'))) as attach, \
                     mock.patch.object(c, 'prepare_executor', side_effect=prepare) as prepared, \
                     mock.patch.object(c, 'launch_executor', return_value={'returncode': step['expected_exit']}) as launched:
                    operation.perform(action, step)
                prepared.assert_called_once(); launched.assert_called_once_with(directory, host)
                if kind == 'resume_one':
                    operation.core['m'].Journal.assert_called_once_with(host.root / 'controller')
                    operation.core['recovery_phases'].checkpoint_from_inspection.assert_called_once_with(
                        host.spec, {'inert': 'inspection'}, core_ledger)
                    operation.core['production_recovery_driver'].expected_heads.assert_called_once_with(checkpoint)
                    attach.assert_not_called()
                elif kind == 'cleanup': attach.assert_called_once_with(host, operation.core)
                else: attach.assert_not_called()
                rows = ledger.reload(); claim, completed = rows[-2:]
                self.assertEqual([claim['type'], completed['type']], ['claim', 'result'])
                self.assertEqual(completed['claim_sha256'], r.digest(claim))
                self.assertEqual(ledger.completed()[action], {'request': request_ref, 'exit': exit_ref})


class Coordination(unittest.TestCase):
    def fixture(self):
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve(); control = root / 'control.json'; lease = root / 'lease.json'
        state = {'schema': 'steward_control_state_v1', 'schema_version': 1, 'paused': True,
                 'pause_generation': 528, 'actor': 'codex-mlx-upgrade'}
        reference = r.write(control, state)
        release = {'coordination': {'control': reference, 'lease_path': str(lease), 'paused': True,
                   'pause_generation': 528, 'actor': 'codex-mlx-upgrade', 'lease_absent': True}}
        self.enterContext(mock.patch.object(c, 'CONTROL', control))
        self.enterContext(mock.patch.object(c, 'LEASE', lease))
        return control, lease, state, release

    def test_current_exact_pause_with_no_lease_is_recorded_without_mutation(self):
        control, lease, state, release = self.fixture(); before = control.read_bytes()
        observed = c.validate_coordination(release)
        self.assertEqual(observed['coordination'], release['coordination'])
        self.assertGreater(observed['observed_wall_time_ns'], 0)
        self.assertEqual(control.read_bytes(), before); self.assertFalse(lease.exists())

    def test_foreign_pause_or_reference_refuses(self):
        control, lease, state, release = self.fixture()
        for changed in ({'paused': False}, {'pause_generation': 529}, {'actor': 'other'}, {'schema': 'other'}):
            with self.subTest(changed=changed), \
                 mock.patch.object(c, 'check_reference', return_value={**state, **changed}), \
                 self.assertRaisesRegex(ValueError, 'ownership differs'):
                c.validate_coordination(release)
        for field, value in (('control', {'path': '/not-the-fixed-path', 'sha256': '0'*64}),
                             ('lease_path', '/not-the-fixed-lease'), ('pause_generation', 529), ('actor', 'other')):
            altered = copy.deepcopy(release); altered['coordination'][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'binding differs'):
                c.validate_coordination(altered)
        changed = copy.deepcopy(release); changed['coordination']['control']['sha256'] = '0'*64
        with self.assertRaises(ValueError): c.validate_coordination(changed)

    def test_any_retained_or_dangling_lease_refuses_without_expiry_or_repair(self):
        control, lease, state, release = self.fixture()
        r.write(lease, {'expires_at_unix': 0, 'inert': 'even an expired record is not ignored'})
        before = lease.read_bytes()
        with self.assertRaisesRegex(ValueError, 'lease exists'): c.validate_coordination(release)
        self.assertEqual(lease.read_bytes(), before)
        lease.unlink(); lease.symlink_to('missing-target')
        with self.assertRaisesRegex(ValueError, 'lease exists'): c.validate_coordination(release)
        self.assertTrue(lease.is_symlink())

    def test_lease_appearing_during_reference_read_refuses(self):
        control, lease, state, release = self.fixture()
        def read_and_arrive(reference):
            value = r.check_reference(reference); r.write(lease, {'inert': 'arrived'}); return value
        with mock.patch.object(c, 'check_reference', side_effect=read_and_arrive), \
             self.assertRaisesRegex(ValueError, 'appeared'):
            c.validate_coordination(release)


class RetainedFiles(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.root = Path(self.tmp.name).resolve()
        self.addCleanup(self.tmp.cleanup)

    def test_inventory_excludes_only_two_empty_root_locks(self):
        r.write(self.root / 'one.json', {'value': 1})
        for name in result.LOCK_NAMES: (self.root / name).write_bytes(b'')
        (self.root / 'nested').mkdir(mode=0o700)
        (self.root / 'nested/execution.lock').write_bytes(b'nested file is evidence')
        inventory, excluded = result.inventory(self.root)
        self.assertEqual(excluded, sorted(str(self.root / name) for name in result.LOCK_NAMES))
        self.assertEqual(set(inventory), {str(self.root / 'one.json'), str(self.root / 'nested/execution.lock')})

    def test_inventory_rejects_nonempty_lock_symlink_or_alias(self):
        (self.root / 'setup.lock').write_bytes(b'not a lock')
        with self.assertRaises(ValueError): result.inventory(self.root)
        (self.root / 'setup.lock').unlink()
        r.write(self.root / 'one.json', {'value': 1})
        (self.root / 'alias').symlink_to('one.json')
        with self.assertRaises(ValueError): result.inventory(self.root)

    def test_identity_inventory_excludes_only_exact_parent_and_rejects_pid_reuse(self):
        parent, child = identity(100), identity(101)
        r.write(self.root / 'identities.json', {'parent': parent, 'children': [child, child]})
        self.assertEqual(result.collect_identities(self.root, parent), {'101': child})
        r.write(self.root / 'different.json', {'identity': {**child, 'started_at': 'another birth'}})
        with self.assertRaises(ValueError): result.collect_identities(self.root, parent)

    def test_parent_same_pid_different_identity_cannot_be_hidden(self):
        parent = identity(100)
        r.write(self.root / 'different.json', {'identity': {**parent, 'argv': ['foreign']}})
        with self.assertRaises(ValueError): result.collect_identities(self.root, parent)

    def test_unexpected_exit_rejected_even_if_reaped(self):
        directory = self.root / 'executors/one'; directory.mkdir(parents=True, mode=0o700)
        os.chmod(directory.parent, 0o700)
        step = {'id': 'one', 'expected_exit': 75, 'kind': 'fresh', 'injection': 'exact'}
        request = r.write(directory / 'request.json', {'step': step, 'root': str(self.root)})
        start = {'identity': identity(101), 'request': request}
        r.write(directory / 'self-start.json', start)
        r.write(directory / 'launch.json', {})
        r.write(directory / 'child-created.json', {'pid': 101})
        r.write(directory / 'exit.json', {'start': start, 'pid': 101, 'returncode': 0,
                                         'reaped': True, 'timed_out': False, 'signals_sent': 0})
        with self.assertRaises(ValueError): result.executor_records(self.root, {'executor_plan': [step]})

    def test75_needs_exact_manifest_match_and_original_boundary(self):
        point = {'id': 'owned', 'stream': 'host', 'event': 'host:debug_result', 'match': {'role': 'provider'}}
        manifest = {'schema': 'owned_fixture_injections_v1', 'scope': 'owned_fixture_only',
                    'production_qualified': False, 'points': [point]}
        manifest_ref = r.write(self.root / 'injections.json', manifest)
        release_ref = r.write(self.root / 'release.json', {'injections': manifest_ref, 'fixture': {'root': str(self.root)}})
        boundary = r.write(self.root / 'boundary.json', {'role': 'provider', 'returncode': 0})
        start = {'release': release_ref, 'identity': identity(101)}
        exited = {'start': start, 'reaped': True, 'returncode': 75, 'signals_sent': 0, 'timed_out': False}
        ack = {'schema': 'owned_fixture_injection_ack_v1', 'point': point, 'executor_identity': start['identity'],
               'expected_exit_code': 75, 'signals_sent_by_injection': 0, 'original_validation_replaced': False,
               'manifest': manifest_ref, 'boundary': boundary}
        self.assertTrue(injection.verify_executor_exit(start, exited, ack, point, release_ref)['verified'])
        for change in ({'original_validation_replaced': True}, {'signals_sent_by_injection': 1},
                       {'manifest': boundary}, {'point': {**point, 'id': 'other'}}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                injection.verify_executor_exit(start, exited, {**ack, **change}, point, release_ref)


if __name__ == '__main__': unittest.main()
