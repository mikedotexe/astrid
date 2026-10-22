"""Private-temp lock tests only; no child, signal, network or service calls."""
import copy
import fcntl
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import reservoir_maintenance as m
import transaction_execution as x
import recovery_ledger as ledger
from test_production_controller import legacy


class ExecutionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name).resolve()
        self.plan = legacy.fake_plan()
        self.executor = {'pid': os.getpid(), 'started_at': 'inert-executor-start', 'argv': ['inert-executor']}

    def session(self, mode='resume_one'):
        return x.ExecutionSession(self.root, self.plan, self.executor, mode)

    def test_same_process_and_independent_lock_fd_exclusion(self):
        with self.session() as active:
            self.assertIs(x.require_active(self.root, self.plan), active)
            with self.assertRaisesRegex(m.Refusal, 'already executing'):
                with self.session(): self.fail('entered twice')
            fd = os.open(self.root / 'execution.lock', os.O_RDWR)
            try:
                with self.assertRaises(BlockingIOError): fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            finally: os.close(fd)
        with self.session(): pass
        self.assertEqual(len(list((self.root / 'execution-sessions').glob('*-begin.json'))), 2)

    def test_exception_has_durable_identity_without_success_inference(self):
        with self.assertRaisesRegex(RuntimeError, 'injected'):
            with self.session() as active:
                begin = copy.deepcopy(active.begin)
                raise RuntimeError('injected')
        path = self.root / 'execution-sessions' / (begin['session_id'] + '-end.json')
        end = m.read_record(path)
        self.assertEqual(end['begin_sha256'], m.digest(begin))
        self.assertEqual(end['error_type'], 'RuntimeError')
        self.assertIs(end['external_action_success_inferred'], False)
        with self.assertRaisesRegex(m.Refusal, 'no current'):
            x.require_active(self.root, self.plan)

    def test_foreign_executor_and_shared_or_symlink_lock_refused(self):
        with self.assertRaisesRegex(m.Refusal, 'not this process'):
            x.ExecutionSession(self.root, self.plan, dict(self.executor, pid=os.getpid()+1), 'fresh')
        target = self.root / 'other'; target.write_text('')
        (self.root / 'execution.lock').symlink_to(target)
        with self.assertRaises(OSError):
            with self.session(): pass
        (self.root / 'execution.lock').unlink()
        (self.root / 'execution.lock').write_text(''); (self.root / 'execution.lock').chmod(0o644)
        with self.assertRaisesRegex(m.Refusal, 'foreign/shared'):
            with self.session(): pass

    def test_begin_write_failure_releases_lock_and_never_enters(self):
        original = m.write_new
        def fail(path, value):
            if str(path).endswith('-begin.json'): raise OSError('durability failure')
            return original(path, value)
        with patch.object(m, 'write_new', side_effect=fail), self.assertRaises(OSError):
            with self.session(): self.fail('entered')
        with self.session(): pass

    def test_replaced_lock_is_detected_before_action(self):
        with self.assertRaisesRegex(m.Refusal, 'replaced'):
            with self.session():
                (self.root / 'execution.lock').rename(self.root / 'retained-lock')
                (self.root / 'execution.lock').write_text('')
                x.require_active(self.root, self.plan)

    def test_ledger_requires_active_lock_and_retained_exact_executor(self):
        with self.assertRaisesRegex(m.Refusal, 'no current'):
            ledger.RecoveryLedger(self.root, self.plan, 'a'*64, 'b'*64)
        with self.session() as active:
            log = ledger.RecoveryLedger(self.root, self.plan, 'a'*64, 'b'*64)
            row = log.append('action_claim', action_id='preflight')
            self.assertEqual(row['executor_session'], active.begin)
        self.assertEqual(len(ledger.read_ledger(self.root, self.plan)['records']), 1)
        begin = self.root / 'execution-sessions' / (row['executor_session']['session_id']+'-begin.json')
        begin.write_text('{}')
        with self.assertRaisesRegex(m.Refusal, 'begin record differs'):
            ledger.read_ledger(self.root, self.plan)

    def test_plan_boot_root_mode_session_mutations_are_refused(self):
        with self.session() as active: value = copy.deepcopy(active.begin)
        for key, mutation in [('host_binding', {'uid':501,'boot_uuid':'other'}), ('root', '/other'),
                              ('mode','fresh'), ('session_id','../outside'), ('plan_sha256','f'*64),
                              ('started_wall_time_ns',0), ('executor_identity', {'pid':0})]:
            bad = copy.deepcopy(value); bad[key] = mutation
            with self.subTest(key=key), self.assertRaises((m.Refusal, KeyError, TypeError)):
                ledger.session_shape(bad, self.root, self.plan)

    def test_fresh_owned_fixture_and_resume_share_same_lock(self):
        host = legacy.FakeHost(self.plan); host.scope = 'owned_fixture'
        host.executor_identity = lambda: self.executor
        for role in m.ROLES:
            host.roles[role]['target'] = f"gui/501/{m.FIXTURE_PREFIX}{self.plan['transaction_id']}.{role}"
        journal = m.Journal(self.root / 'controller', plan=self.plan)
        original_event = host.event
        def guarded_event(op, role=None):
            x.require_active(self.root, self.plan)
            with self.assertRaisesRegex(m.Refusal, 'already executing'):
                with self.session(): pass
            original_event(op, role)
        host.event = guarded_event
        result = m.Controller(journal, host).run()
        self.assertEqual(result['status'], 'verified')
        sessions = list((self.root/'execution-sessions').glob('*-begin.json'))
        self.assertEqual(m.read_record(sessions[0])['mode'], 'fresh')

    def test_stale_fresh_journal_refuses_before_second_host_action(self):
        host = legacy.FakeHost(self.plan); host.scope = 'owned_fixture'; host.executor_identity = lambda: self.executor
        for role in m.ROLES: host.roles[role]['target'] = f"gui/501/{m.FIXTURE_PREFIX}{self.plan['transaction_id']}.{role}"
        first = m.Journal(self.root/'controller', plan=self.plan); stale = m.Journal(first.root)
        m.Controller(first, host).run(); before = list(host.calls)
        with self.assertRaisesRegex(m.Refusal, 'stale fresh Journal'): m.Controller(stale, host).run()
        self.assertEqual(before, host.calls)

    def test_recovery_history_blocks_fresh_even_with_empty_original_journal(self):
        host = legacy.FakeHost(self.plan); host.scope = 'owned_fixture'; host.executor_identity = lambda: self.executor
        journal = m.Journal(self.root/'controller', plan=self.plan)
        with self.session(): ledger.RecoveryLedger(self.root, self.plan, 'a'*64, journal.previous)
        with self.assertRaisesRegex(m.Refusal, 'recovery history exists'): m.Controller(journal, host).run()
        self.assertEqual(host.calls, [])


if __name__ == '__main__': unittest.main(verbosity=2)
