"""Inert source/record tests. Never invokes the native adapters or a child."""
import contextlib
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

import owned_fixture_records as r
import owned_fixture_setup as setup
import owned_fixture_cleanup as cleanup
import owned_fixture_injection as injection
import owned_fixture_coordinator as coordinator


def ident(pid=None):
    return {'pid': os.getpid() if pid is None else pid, 'started_at': 'fixture-only-start', 'argv': ['fixture-python']}


class Records(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.root = Path(self.tmp.name).resolve()
    def tearDown(self): self.tmp.cleanup()

    def test_stale_append_refused(self):
        one = r.Ledger(self.root, 'records', {'input': 1}, create=True)
        two = r.Ledger(self.root, 'records', {'input': 1})
        one.append('claim', 'first')
        with self.assertRaisesRegex(ValueError, 'changed'): two.append('claim', 'second')

    def test_failed_claim_needs_reconciliation(self):
        ledger = r.Ledger(self.root, 'records', {}, create=True)
        claim = ledger.append('claim', 'first')
        ledger.append('failed', 'first', claim_sha256=r.digest(claim))
        with self.assertRaises(ValueError): ledger.append('result', 'first', claim_sha256=r.digest(claim), result={})
        with self.assertRaises(ValueError): ledger.append('claim', 'first')
        adopted = ledger.append('reconcile_claim', 'first')
        ledger.append('reconciled', 'first', claim_sha256=r.digest(adopted), resolves_claims=ledger.outstanding('first'),
                      proof={'passed': True}, result={'effect': 'observed'})
        self.assertEqual(ledger.completed(), {'first': {'effect': 'observed'}})

    def test_incomplete_reconciliation_closure_refused(self):
        ledger = r.Ledger(self.root, 'records', {}, create=True)
        ledger.append('claim', 'first'); row = ledger.append('reconcile_claim', 'first')
        with self.assertRaises(ValueError):
            ledger.append('reconciled', 'first', claim_sha256=r.digest(row), resolves_claims=[r.digest(row)],
                          proof={'passed': True}, result={})

    def test_record_alias_and_partial_chain_rejected(self):
        r.write(self.root / 'value.json', {'value': 1}); (self.root / 'alias.json').symlink_to('value.json')
        with self.assertRaises(ValueError): r.read(self.root / 'alias.json')
        ledger = r.Ledger(self.root, 'records', {}, create=True); ledger.append('claim', 'first')
        (ledger.path / 'event-00001.json').rename(ledger.path / 'event-00002.json')
        with self.assertRaises(ValueError): ledger.reload()


class SetupHost:
    def __init__(self, root, owner):
        self.root = root; self.owner = owner; self.host_binding = owner['host_binding']
        self.current = ident(); self.processes = {}; self.calls = []
        self.env = types.SimpleNamespace(command_matches=lambda a, b: a == b)
        self.io = types.SimpleNamespace(process=lambda pid: self.processes.get(pid))
        self.views = {role: {'job_loaded': False, 'process': None, 'listener_pids': [], 'ready': False}
                      for role in ('provider', 'reservoir')}
    def executor_identity(self): return self.current
    def target(self, role): return self.owner['targets'][role]
    def argv(self, role): return ['fixture-python', role]
    def validate_setup_owner(self, owner, require_executor=True):
        r.require(owner == self.owner, 'owner differs')
        if require_executor and self.current != owner['coordinator_identity']:
            successor = r.read(self.root / 'setup-successor.json')
            r.require(successor['executor_identity'] == self.current, 'wrong successor')
    def setup_observe(self, role): return self.views[role]
    def setup_bootstrap(self, role, claim):
        self.calls.append(role)
        identity = ident(900 + len(self.calls)); identity['argv'] = self.argv(role)
        self.views[role] = {'job_loaded': True, 'process': identity, 'listener_pids': [identity['pid']], 'ready': True}
        return {'identity': identity, 'ready': True}
    def setup_reconcile(self, role, claim):
        return {'passed': True, 'action': 'bootstrap:' + role, 'original_claim_sha256': r.digest(claim),
                'result': {'identity': self.views[role]['process'], 'ready': self.views[role]['ready']}}


class Setup(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.parent = Path(self.tmp.name).resolve()
        self.token = 'a' * 32; self.root = self.parent / ('owned-recovery-' + self.token)
        self.binding = {'uid': os.getuid(), 'boot_uuid': 'fixture-boot', 'hostname': 'fixture', 'os_version': 'fixture'}
        sources = []
        for name in ('maintenance_fixture_worker.py', 'model_environment.py'):
            path = self.parent / name; path.write_text('# inert test bytes\n'); sources.append(path)
        self.release = {'schema': 'owned_recovery_fixture_release_v1', 'approved': True, 'scope': 'owned_fixture_only',
            'production_qualified': False, 'final_release_acceptance': False, 'host_binding': self.binding,
            'fixture': {'root': str(self.root), 'token': self.token,
                        'targets': {role: r.target(os.getuid(), self.token, role) for role in ('provider', 'reservoir', 'pin')}},
            'setup_plan': ['bootstrap:reservoir', 'bootstrap:provider'],
            'setup_successor_policy': {'max_successors': 1, 'allowed_actions': ['bootstrap:reservoir', 'bootstrap:provider'],
                                       'no_attempt_replay': True},
            'artifacts': {str(p): r.reference(p)['sha256'] for p in sources}}
        r.write(self.parent / 'release.json', self.release)
        self.owner, _ = setup.create_layout(self.root, self.token, r.reference(self.parent / 'release.json'),
                        ident(), self.binding, 'fixture-python', *sources)
        self.host = SetupHost(self.root, self.owner)
        for role in ('provider', 'reservoir'): (self.root / (role + '.plist')).write_text('inert plist bytes')
    def tearDown(self): self.tmp.cleanup()

    def test_setup_guard_requires_current_exact_held_descriptor(self):
        with self.assertRaises(ValueError): setup.require_setup_active(self.root)
        with setup.SetupSession(self.root):
            setup.require_setup_active(self.root)
            with self.assertRaises(ValueError), setup.SetupSession(self.root): pass
            with mock.patch.object(setup.os, 'getpid', return_value=999999):
                with self.assertRaises(ValueError): setup.require_setup_active(self.root)
            (self.root / 'setup.lock').rename(self.root / 'retained-setup.lock')
            (self.root / 'setup.lock').write_text('replacement')
            with self.assertRaises(ValueError): setup.require_setup_active(self.root)
        with self.assertRaises(ValueError): setup.require_setup_active(self.root)

    def test_initial_bootstrap_once_and_actual_executor(self):
        with self.assertRaises(ValueError):
            setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident(444))
        setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident())
        with self.assertRaises(ValueError):
            setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident())
        self.assertEqual(self.host.calls, ['reservoir'])

    def test_lost_result_can_adopt_but_never_rebootstrap(self):
        original = self.host.setup_bootstrap
        def lost(role, claim): original(role, claim); raise RuntimeError('lost result')
        self.host.setup_bootstrap = lost
        with self.assertRaises(RuntimeError):
            setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident())
        with self.assertRaises(ValueError):
            setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident())
        setup.bootstrap_one(self.root, self.host, 'reservoir', mode='reconcile', executor=ident())
        self.assertEqual(self.host.calls, ['reservoir'])

    def test_successor_only_one_next_unattempted_action(self):
        setup.bootstrap_one(self.root, self.host, 'reservoir', mode='execute', executor=ident())
        # Retain original owner identity; simulate a later process for pure ownership checks.
        prior = self.owner['coordinator_identity']; new = ident(777)
        self.host.current = new
        _, ledger = setup.setup_ledger(self.root)
        with mock.patch.object(setup.os, 'getpid', return_value=777):
            result = setup.authorize_successor(self.root, self.host, expected_head=ledger.head,
                                              next_action='bootstrap:provider')
        self.assertEqual(result['prior_executor'], prior)
        self.assertEqual(r.read(self.root / 'setup-owner.json'), self.owner)
        with self.assertRaises(ValueError):
            setup.authorize_successor(self.root, self.host, expected_head=ledger.head, next_action='bootstrap:provider')

    def test_successor_rejects_pending_attempt_stale_head_or_live_prior(self):
        _, ledger = setup.setup_ledger(self.root)
        self.host.current = ident(777); self.host.processes[self.owner['coordinator_identity']['pid']] = self.owner['coordinator_identity']
        with mock.patch.object(setup.os, 'getpid', return_value=777):
            with self.assertRaises(ValueError): setup.authorize_successor(self.root, self.host, expected_head=ledger.head, next_action='bootstrap:reservoir')
            self.host.processes.clear()
            with self.assertRaises(ValueError): setup.authorize_successor(self.root, self.host, expected_head='stale', next_action='bootstrap:reservoir')
            ledger.append('claim', 'bootstrap:reservoir')
            with self.assertRaises(ValueError): setup.authorize_successor(self.root, self.host, expected_head=ledger.head, next_action='bootstrap:provider')


class InjectionTests(unittest.TestCase):
    def test_ack_is_durable_before_only_self_exit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve(); (root / 'executors').mkdir(mode=0o700)
            execution = root / 'executors/one'; execution.mkdir(mode=0o700)
            point = {'id': 'debug', 'stream': 'host', 'event': 'host:debug_result', 'match': {'role': 'provider'}}
            manifest = {'schema': 'owned_fixture_injections_v1', 'scope': 'owned_fixture_only',
                        'production_qualified': False, 'points': [point]}
            r.write(root / 'injections.json', manifest)
            host = types.SimpleNamespace(scope='owned_fixture', root=root, executor_identity=ident)
            seen = []
            def own_exit(code):
                seen.append((code, r.read(execution / 'injection-ack.json')))
                raise RuntimeError('fake self exit')
            item = injection.Injection(host, r.reference(root / 'injections.json'), 'debug', execution, exit_self=own_exit)
            boundary = r.write(root / 'debug.json', {'role': 'provider', 'returncode': 0})
            with self.assertRaisesRegex(RuntimeError, 'fake self exit'): item.host_boundary('host:debug_result', boundary)
            self.assertEqual(seen[0][0], 75); self.assertEqual(seen[0][1]['boundary'], boundary)
            with self.assertRaises(ValueError): item.host_boundary('host:debug_result', boundary)

    def test_production_and_unknown_stream_refuse(self):
        with self.assertRaises(ValueError):
            injection.validate_manifest({'schema': 'owned_fixture_injections_v1', 'scope': 'owned_fixture_only',
                'production_qualified': False, 'points': [{'id': 'x', 'stream': 'cleanup', 'event': 'x', 'match': {}}]})


class CleanupResults(unittest.TestCase):
    def test_independent_final_state_and_two_checkins_required(self):
        context = {'expected_ticks': {'fixture_astrid': {'ticks': 22}, 'fixture_minime': {'ticks': 7}}}
        value = {'passed': True, 'both_jobs_absent': True, 'all_owned_pids_absent': True, 'all_ports_unowned': True,
                 'final_state': context['expected_ticks'], 'main_backup_unchanged': True, 'unique_applied_checkins': 2,
                 'force_used': False}
        cleanup.validate_result('complete', value, context, {})
        for key, bad in [('unique_applied_checkins', 1), ('main_backup_unchanged', False), ('force_used', True)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                cleanup.validate_result('complete', {**value, key: bad}, context, {})

    def test_disabled_coordinator_never_spawns(self):
        with mock.patch.object(coordinator.subprocess, 'Popen', side_effect=AssertionError('must not spawn')):
            with self.assertRaisesRegex(ValueError, 'disabled'): coordinator.launch_executor('/unused', None)
            with self.assertRaisesRegex(ValueError, 'disabled'): coordinator.execute_request('/unused')


class PairPreparation(unittest.TestCase):
    def test_live_lock_holder_binds_retained_start_instead_of_nonexistent_exit(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve(); (root / 'executors').mkdir(mode=0o700)
            parent = root / 'executors'; holder = parent / 'holder'; holder.mkdir(mode=0o700)
            pair = [{'id': 'holder', 'kind': 'lock_hold', 'expected_exit': 0},
                    {'id': 'refusal', 'kind': 'lock_refusal', 'expected_exit': 76}]
            release = {'fixture': {'root': str(root)}, 'executor_plan': pair, 'python': '/fixture-python'}
            r.write(root / 'release.json', release); r.write(root / 'plan.json', {'fixture': True})
            r.write(root / 'setup-complete.json', {'plan_ref': r.reference(root / 'plan.json')})
            r.write(holder / 'request.json', {'step': pair[0]})
            request_ref = r.reference(holder / 'request.json'); held_identity = ident(901)
            r.write(holder / 'self-start.json', {'identity': held_identity, 'request': request_ref})
            r.write(holder / 'launch.json', {'request': request_ref})
            r.write(holder / 'lock-held.json', {'identity': held_identity, 'request': request_ref})
            host = types.SimpleNamespace(check_owned=lambda: None, executor_identity=ident, root=root,
                         owner={'release_ref': r.reference(root / 'release.json')}, executable=Path('/fixture-python'),
                         io=types.SimpleNamespace(process=lambda pid: held_identity if pid == 901 else None))
            with mock.patch.object(coordinator, 'validate_release', return_value=release):
                prepared = coordinator.prepare_executor(root, r.reference(root / 'release.json'),
                                                        'refusal', host, expected_heads={})
            previous = r.read(prepared / 'request.json')['previous_steps']
            self.assertEqual(previous, [{'kind': 'live_nonmutating_lock_holder', 'step_id': 'holder',
                  'held': r.reference(holder / 'lock-held.json'),
                  'start': r.reference(holder / 'self-start.json'), 'launch': r.reference(holder / 'launch.json')}])
            self.assertFalse((holder / 'exit.json').exists())
            # The exception cannot authorize a later mutation while either executor is alive.
            release['executor_plan'].append({'id': 'later', 'kind': 'fresh', 'expected_exit': 0})
            with mock.patch.object(coordinator, 'validate_release', return_value=release):
                with self.assertRaises(FileNotFoundError):
                    coordinator.prepare_executor(root, r.reference(root / 'release.json'),
                                                 'later', host, expected_heads={})

    def test_python_selection_must_match_exact_reviewed_framework_family(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve(); version = root / 'Python.framework/Versions/3.14'
            wrapper = version / 'bin/python3.14'; actual = version / 'Resources/Python.app/Contents/MacOS/Python'
            wrapper.parent.mkdir(parents=True); actual.parent.mkdir(parents=True)
            wrapper.write_bytes(b'inert launcher'); actual.write_bytes(b'inert runtime')
            alias = root / 'python'; alias.symlink_to(wrapper)
            release = {'python': str(alias), 'python_files': {str(p): r.reference(p)['sha256'] for p in (wrapper, actual)}}
            coordinator.validate_python(release)
            unrelated = root / 'unrelated'; unrelated.write_bytes(b'other inert executable')
            with self.assertRaisesRegex(ValueError, 'family'):
                coordinator.validate_python({**release, 'python': str(unrelated)})
            with self.assertRaisesRegex(ValueError, 'family'):
                coordinator.validate_python({**release, 'python_files': {str(wrapper): r.reference(wrapper)['sha256']}})

    def test_alternate_self_consistent_release_ref_is_not_the_owned_release(self):
        host = types.SimpleNamespace(check_owned=lambda: None, owner={'release_ref': {'path': '/owned', 'sha256': 'a'}})
        with mock.patch.object(coordinator, 'validate_release', side_effect=AssertionError('must reject before selected file')):
            with self.assertRaisesRegex(ValueError, 'retained owner'):
                coordinator.owned_release(host, {'path': '/other', 'sha256': 'b'})


if __name__ == '__main__': unittest.main()
