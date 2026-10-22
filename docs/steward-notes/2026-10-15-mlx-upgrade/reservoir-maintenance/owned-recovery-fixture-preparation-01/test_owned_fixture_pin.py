"""Finite private-file pin tests with inert IO only."""
import contextlib
import copy
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

import owned_fixture_records as r
import owned_fixture_pin as pin
import owned_fixture_plan as plan
import owned_fixture_injection as injection


class PinHost:
    scope = 'owned_fixture'
    def __init__(self, root):
        self.root = root; self.uid = os.getuid(); self.token = 'b' * 32; self.executable = Path('/fixture-python')
        self.plan = {'owned': 'inert-test'}; self.checks = 0
        self.observation_ticks = 0; self.on_wait = lambda: None
        self.env = types.SimpleNamespace(command_matches=lambda a, b: a == b)
    def check_owned(self): self.checks += 1
    def executor_identity(self): return {'pid': os.getpid(), 'started_at': 'inert', 'argv': ['inert']}
    def wait(self, predicate, label, timeout=90):
        for _ in range(timeout * 10):
            result = predicate()
            if result:return result
            self.observation_ticks += 1; self.on_wait()
        raise ValueError(label + ' timed out; no escalation')


class PinIO:
    def __init__(self): self.loaded = False; self.boots = self.removals = 0; self.running = False
    def pin_observe(self, target, pid):
        return {'job_loaded': self.loaded, 'process': {'pid': pid} if self.running else None,
                'recorded_pid_absent': not self.running, 'runs': 1 if self.loaded else None,
                'last_exit_code': 0 if self.loaded else None, 'listener_pids': []}
    def pin_bootstrap(self, path, target): self.boots += 1; self.loaded = True; return {'returncode': 0}
    def pin_remove_absent(self, target, pid): self.removals += 1; self.loaded = False; return {'returncode': 0}


class Tests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.root = Path(self.tmp.name).resolve()
        self.host = PinHost(self.root); self.io = PinIO()
        for name in ('owned_pin_sentinel.py', 'owned_fixture_records.py'): (self.root / name).write_text('# inert test bytes\n')
        self.core = {'transaction_execution': types.SimpleNamespace(ExecutionSession=lambda *_: contextlib.nullcontext())}
        pin.initialize(self.host, self.core)
    def tearDown(self): self.tmp.cleanup()
    def head(self): return r.Ledger(self.root, 'pin', r.read(self.root / 'pin/inputs.json')['binding']).head
    def step(self, action, mode='execute', hook=None):
        return pin.step(self.host, self.core, self.io, expected_head=self.head(), action=action, mode=mode, boundary_hook=hook)

    def test_replace_interruption_adopts_exact_bytes_without_second_replace(self):
        calls = []; actual = pin.os.replace
        def replace(*args): calls.append(args); return actual(*args)
        def interrupted(*_): raise RuntimeError('interrupted after exact atomic replacement')
        with mock.patch.object(pin.os, 'replace', side_effect=replace):
            with self.assertRaises(RuntimeError): self.step('replace_plist', hook=interrupted)
            with self.assertRaises(ValueError): self.step('replace_plist')
            self.step('replace_plist', 'reconcile')
        self.assertEqual(len(calls), 1)
        self.assertEqual(r.raw(self.root / 'pin/active.plist'), r.raw(self.root / 'pin/candidate.plist'))

    def test_unknown_bootstrap_is_not_replayed(self):
        self.step('replace_plist')
        def lost(*_): self.io.boots += 1; raise RuntimeError('unknown return')
        self.io.pin_bootstrap = lost
        with self.assertRaises(RuntimeError): self.step('bootstrap')
        with self.assertRaises(ValueError): self.step('bootstrap')
        with self.assertRaises(FileNotFoundError): self.step('bootstrap', 'reconcile')
        self.assertEqual(self.io.boots, 1)

    def test_complete_private_sequence_has_distinct_scope(self):
        self.step('replace_plist'); self.step('bootstrap')
        argv = pin.settings(self.host, self.root / 'pin/model-b.sentinel')['ProgramArguments']
        r.write(self.root / 'pin/sentinel-exit-intent.json', {'token': self.host.token, 'argv': argv, 'pid': 999,
            'model_argument': str(self.root / 'pin/model-b.sentinel'), 'model_opened': False,
            'listener_opened': False, 'numeric_imported': False})
        self.step('verify_exit'); self.step('remove_absent_job'); result = self.step('complete')['result']
        self.assertFalse(result['production_qualified']); self.assertFalse(result['production_pin_schedule_executed'])
        self.assertEqual((self.io.boots, self.io.removals), (1, 1)); self.assertGreater(self.host.checks, 0)

    def test_running_sentinel_cannot_be_removed(self):
        self.step('replace_plist'); self.step('bootstrap'); self.io.running = True
        r.write(self.root / 'pin/sentinel-exit-intent.json', {'token': self.host.token,
          'argv': pin.settings(self.host, self.root / 'pin/model-b.sentinel')['ProgramArguments'], 'pid': 999,
          'model_argument': str(self.root / 'pin/model-b.sentinel'), 'model_opened': False,
          'listener_opened': False, 'numeric_imported': False})
        with self.assertRaises(ValueError): self.step('verify_exit')
        self.assertEqual(self.io.removals, 0)

    def test_initial_pin_receipt_publication_wait_does_not_repeat_bootstrap(self):
        self.step('replace_plist'); self.step('bootstrap')
        def publish():
            if self.host.observation_ticks == 2:
                r.write(self.root / 'pin/sentinel-exit-intent.json', {'token': self.host.token,
                  'argv': pin.settings(self.host, self.root / 'pin/model-b.sentinel')['ProgramArguments'], 'pid': 999,
                  'model_argument': str(self.root / 'pin/model-b.sentinel'), 'model_opened': False,
                  'listener_opened': False, 'numeric_imported': False})
        self.host.on_wait = publish
        result = self.step('verify_exit')['result']
        self.assertTrue(result['passed']); self.assertEqual(self.host.observation_ticks, 2)
        self.assertEqual((self.io.boots, self.io.removals), (1, 0))

    def test_missing_pin_receipt_times_out_without_control_retry(self):
        self.step('replace_plist'); self.step('bootstrap')
        with self.assertRaisesRegex(ValueError, 'timed out'): self.step('verify_exit')
        self.assertEqual((self.io.boots, self.io.removals), (1, 0))

    def test_pre_action_initialization_adopts_identical_bytes(self):
        (self.root / 'pin/inputs.json').unlink()
        pin.initialize(self.host, self.core)
        self.assertTrue((self.root / 'pin/inputs.json').exists())
        (self.root / 'pin/active.plist').write_bytes(b'foreign')
        with self.assertRaises(ValueError): pin.initialize(self.host, self.core)

    def test_proposed_schedule_is_finite_and_has_exact_interruption_pairs(self):
        schedule = plan.proposed_schedule(); injection.validate_manifest(schedule['injections'])
        steps = schedule['executor_plan']; self.assertLessEqual(len(steps), 96)
        self.assertEqual([s['kind'] for s in steps[:2]], ['lock_hold', 'lock_refusal'])
        self.assertEqual(len({s['id'] for s in steps}), len(steps))
        for index, row in enumerate(steps):
            if row['injection']:
                following = steps[index + 1]
                self.assertEqual(following['action_id'], row['action_id'])
                self.assertEqual(following['mode'], 'reconcile')
                self.assertIsNone(following['injection'])
        self.assertFalse(schedule['final_release_acceptance'])


if __name__ == '__main__': unittest.main()
