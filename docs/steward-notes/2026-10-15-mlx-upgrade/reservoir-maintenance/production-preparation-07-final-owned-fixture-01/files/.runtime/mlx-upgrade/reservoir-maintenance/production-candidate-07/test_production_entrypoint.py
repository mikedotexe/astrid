"""Guarded stdlib entrypoint boundary tests; all Host/OS calls are fakes."""
from contextlib import ExitStack
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

import production_entrypoint as e
import production_native_attempt as native_attempt


class RecordTests(unittest.TestCase):
    def test_exact_record_hash_and_special_path_refusals(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            path = root / 'proof.json'; raw = b'{"ok":true}\n'; path.write_bytes(raw)
            sha = hashlib.sha256(raw).hexdigest()
            self.assertEqual(e.record(path, sha), {'ok': True})
            with self.assertRaises(e.m.Refusal): e.record(path, '0' * 64)
            alias = root / 'alias.json'; alias.symlink_to(path)
            with self.assertRaises(e.m.Refusal): e.record(alias, sha)
            linked = root / 'linked.json'; os.link(path, linked)
            with self.assertRaises(e.m.Refusal): e.record(path, sha)

    def test_nonfinite_or_nonobject_record_refused(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder).resolve() / 'proof.json'
            for raw in (b'{"value":NaN}', b'[]'):
                path.write_bytes(raw)
                with self.subTest(raw=raw), self.assertRaises(e.m.Refusal):
                    e.record(path, hashlib.sha256(raw).hexdigest())

    def test_ordinary_identity_and_terminal_required(self):
        for uid, effective, tty in ((0, 0, True), (501, 0, True), (501, 501, False)):
            with patch.object(e.os, 'getuid', return_value=uid), patch.object(e.os, 'geteuid', return_value=effective), \
                    patch.object(e.sys.stdin, 'isatty', return_value=tty), self.assertRaises(e.m.Refusal):
                e.ordinary_user(interactive=True)


class ClosureTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack(); self.addCleanup(self.stack.close)
        folder = self.stack.enter_context(tempfile.TemporaryDirectory())
        self.root = Path(folder).resolve()
        modules = (e.p, e.h, e.inspection, e.driver, e.ledger, e.phases, e.m,e.native,e.native_attempt)
        for module in modules:
            source = self.root / (module.__name__ + '.py'); source.write_text('# inert fake source\n')
            self.stack.enter_context(patch.object(module, '__file__', str(source)))
        self.extra = self.root / 'additional_helper.py'; self.extra.write_text('# closure member\n')
        hold = self.root / 'hold.py'; hold.write_text('# inert holder\n')
        self.launcher = self.root / 'launcher'; self.launcher.write_bytes(b'fake-launcher')
        self.runtime = self.root / 'framework-runtime'; self.runtime.write_bytes(b'fake-framework-runtime')
        self.stack.enter_context(patch.object(e, 'HERE', self.root))
        self.stack.enter_context(patch.object(e.p, 'HOLD_SOURCE', hold))
        self.stack.enter_context(patch.object(e.h, 'HOLDER_PYTHON', self.launcher))
        self.stack.enter_context(patch.object(e.sys, 'executable', str(self.runtime)))
        self.family = Mock(return_value={self.launcher, self.runtime})
        self.stack.enter_context(patch.object(e.p, 'load_hold', return_value=types.SimpleNamespace(interpreter_paths=self.family)))
        self.spec = {'immutable_files': {str(path): e.p.sha(path) for path in self.root.iterdir()},
                     'input_groups': {'helpers': [str(path) for path in self.root.glob('*.py')]}}

    def test_fixed_origins_full_helpers_and_pinned_framework_family(self):
        e.source_and_interpreter(self.spec)
        self.family.assert_called_once_with(self.launcher)

    def test_missing_helper_or_changed_source_refused(self):
        removed = copy.deepcopy(self.spec)
        removed['input_groups']['helpers'].remove(str(self.extra))
        with self.assertRaisesRegex(e.m.Refusal, 'helper closure changed'): e.source_and_interpreter(removed)
        self.extra.write_text('# changed\n')
        with self.assertRaisesRegex(e.m.Refusal, 'helper closure changed'): e.source_and_interpreter(self.spec)

    def test_foreign_loaded_origin_refused(self):
        foreign = self.root / 'foreign.py'; foreign.write_text('# preloaded alias\n')
        self.spec['immutable_files'][str(foreign)] = e.p.sha(foreign)
        self.spec['input_groups']['helpers'].append(str(foreign))
        with patch.object(e.h, '__file__', str(foreign)), self.assertRaisesRegex(e.m.Refusal, 'loaded helper origin'):
            e.source_and_interpreter(self.spec)

    def test_wrong_interpreter_or_changed_framework_bytes_refused(self):
        with patch.object(e.sys, 'executable', str(self.extra)), self.assertRaisesRegex(e.m.Refusal, 'outside pinned family'):
            e.source_and_interpreter(self.spec)
        self.runtime.write_bytes(b'changed-runtime')
        with self.assertRaisesRegex(e.m.Refusal, 'interpreter changed'): e.source_and_interpreter(self.spec)


class DispatchTests(unittest.TestCase):
    def setUp(self):
        self.stack = ExitStack(); self.addCleanup(self.stack.close)
        folder = self.stack.enter_context(tempfile.TemporaryDirectory())
        self.base = Path(folder).resolve()
        self.transaction = self.base / ('a' * 32)
        self.plan = {'transaction_id': 'a' * 32, 'host_binding': {'uid': 501}}
        self.spec = {'plan': self.plan, 'transaction_root': str(self.transaction)}
        self.path = self.base / 'preparation.json'; self.sha = 'f' * 64
        self.record = self.stack.enter_context(patch.object(e, 'record', return_value=self.spec))
        self.stack.enter_context(patch.object(e, 'ordinary_user'))
        self.stack.enter_context(patch.object(e.os, 'getuid', return_value=501))
        self.stack.enter_context(patch.object(e.m, 'validate_plan', side_effect=lambda plan: copy.deepcopy(plan)))
        self.stack.enter_context(patch.object(e.p, 'CONTROL_ROOT', self.base))
        self.closure = self.stack.enter_context(patch.object(e, 'source_and_interpreter'))
        self.host_type = self.stack.enter_context(patch.object(e.h, 'ProductionHost'))
        self.controller = self.stack.enter_context(patch.object(e.m, 'Controller'))
        self.journal = self.stack.enter_context(patch.object(e.m, 'Journal'))
        self.check = self.stack.enter_context(patch.object(e.p, 'check_only', return_value={'passed': True}))
        self.system = self.stack.enter_context(patch.object(e.p, 'ReadOnlySystem'))
        self.reserve = self.stack.enter_context(patch.object(native_attempt,'reserve',return_value=self.system.return_value))
        self.resume = self.stack.enter_context(patch.object(e.driver, 'resume_one', return_value={'step': 'one'}))
        self.inspector = self.stack.enter_context(patch.object(e.inspection, 'inspect', return_value={'evidence_valid': False}))
        self.stack.enter_context(patch.object(e.inspection, 'ReadOnlyIO'))
        self.stack.enter_context(patch.object(e.h, 'PRODUCTION_EXECUTION_ENABLED', False))
        self.stack.enter_context(patch.object(e.m, 'PRODUCTION_CONTROLLER_ENABLED', False))
        # Defence in depth: any missed fake OS command/signal must fail the test.
        self.stack.enter_context(patch.object(e.h.subprocess, 'run', side_effect=AssertionError('live command forbidden')))
        self.stack.enter_context(patch.object(e.h.os, 'kill', side_effect=AssertionError('signal forbidden')))

    def enable_fakes(self):
        self.stack.enter_context(patch.object(e.h, 'PRODUCTION_EXECUTION_ENABLED', True))
        self.stack.enter_context(patch.object(e.m, 'PRODUCTION_CONTROLLER_ENABLED', True))

    def test_default_is_check_only_without_host_capture(self):
        self.assertEqual(e.run(self.path, self.sha), {'passed': True})
        self.check.assert_called_once(); self.host_type.assert_not_called()
        self.controller.assert_not_called(); self.resume.assert_not_called()

    def test_every_action_mode_blocked_before_preparation_read(self):
        for mode in e.ACTION_MODES:
            with self.subTest(mode=mode), self.assertRaisesRegex(e.m.Refusal, 'remain disabled'):
                e.run(self.path, self.sha, mode=mode)
        self.record.assert_not_called(); self.host_type.assert_not_called(); self.resume.assert_not_called()

    def test_either_disabled_flag_independently_blocks(self):
        for controller_flag, host_flag in ((True, False), (False, True)):
            with patch.object(e.m, 'PRODUCTION_CONTROLLER_ENABLED', controller_flag), \
                    patch.object(e.h, 'PRODUCTION_EXECUTION_ENABLED', host_flag), self.assertRaises(e.m.Refusal):
                e.run(self.path, self.sha, mode='fresh')
        self.record.assert_not_called(); self.host_type.assert_not_called()

    def test_heads_for_nonrecovery_or_missing_recovery_heads_refused(self):
        self.enable_fakes()
        with self.assertRaises(e.m.Refusal): e.run(self.path, self.sha, heads_path=self.path, heads_sha256=self.sha)
        with self.assertRaises(e.m.Refusal): e.run(self.path, self.sha, mode='resume-execute')
        self.host_type.assert_not_called(); self.resume.assert_not_called()

    def test_wrong_transaction_root_or_user_refused_before_host(self):
        self.spec['transaction_root'] = str(self.base / 'wrong')
        with self.assertRaises(e.m.Refusal): e.run(self.path, self.sha)
        self.spec['transaction_root'] = str(self.transaction); self.plan['host_binding']['uid'] = 502
        with self.assertRaises(e.m.Refusal): e.run(self.path, self.sha)
        self.check.assert_not_called(); self.host_type.assert_not_called()

    def test_fresh_never_replays_retained_root(self):
        self.enable_fakes(); self.transaction.mkdir()
        with self.assertRaisesRegex(e.m.Refusal, 'never replay fresh'):
            e.run(self.path, self.sha, mode='fresh')
        self.host_type.assert_not_called(); self.controller.assert_not_called()

    def test_fresh_admission_precedes_capture_and_controller_runs_once(self):
        self.enable_fakes()
        admission = Mock()
        module = types.ModuleType('production_admission'); module.validate_admission = admission
        module.__file__ = str(e.HERE / 'production_admission.py')
        with patch.dict(e.sys.modules, {'production_admission': module}):
            result = e.run(self.path, self.sha, mode='fresh')
        admission.assert_called_once_with(self.spec, self.plan)
        host = self.host_type.return_value
        host.prepare_capture.assert_called_once_with()
        self.assertEqual(host.preparation_path, self.path)
        self.assertEqual(host.journal_path, self.transaction / 'controller')
        self.journal.assert_called_once_with(host.journal_path, plan=self.plan)
        self.controller.return_value.run.assert_called_once_with()
        self.resume.assert_not_called(); self.assertIs(result, self.controller.return_value.run.return_value)

    def test_capture_error_retained_without_controller_or_cleanup(self):
        self.enable_fakes(); host = self.host_type.return_value
        host.prepare_capture.side_effect = e.m.Refusal('partial capture')
        module = types.ModuleType('production_admission'); module.validate_admission = Mock()
        module.__file__ = str(e.HERE / 'production_admission.py')
        with patch.dict(e.sys.modules, {'production_admission': module}), self.assertRaisesRegex(e.m.Refusal, 'partial capture'):
            e.run(self.path, self.sha, mode='fresh')
        self.journal.assert_not_called(); self.controller.assert_not_called(); self.resume.assert_not_called()
        self.assertEqual(host.method_calls, [unittest.mock.call.prepare_capture()])

    def test_changed_preparation_after_capture_blocks_controller(self):
        self.enable_fakes()
        self.record.side_effect = [self.spec, self.spec, dict(self.spec, changed=True)]
        module = types.ModuleType('production_admission'); module.validate_admission = Mock()
        module.__file__ = str(e.HERE / 'production_admission.py')
        with patch.dict(e.sys.modules, {'production_admission': module}), self.assertRaisesRegex(e.m.Refusal, 'after capture'):
            e.run(self.path, self.sha, mode='fresh')
        self.host_type.return_value.prepare_capture.assert_called_once()
        self.controller.assert_not_called(); self.resume.assert_not_called()

    def test_resume_executes_exactly_one_bound_step_without_fresh_capture(self):
        self.enable_fakes()
        heads = {key: 'b' * 64 for key in e.driver.HEAD_KEYS}
        self.record.side_effect = [self.spec, heads, self.spec]
        host = self.host_type.from_verified_recovery.return_value
        host.spec = self.spec; host.plan = self.plan
        result = e.run(self.path, self.sha, mode='resume-reconcile', heads_path=self.base / 'heads.json', heads_sha256='c'*64)
        self.host_type.assert_not_called(); host.prepare_capture.assert_not_called(); self.controller.assert_not_called()
        self.resume.assert_called_once_with(self.journal.return_value, host, expected=heads, mode='reconcile')
        self.assertEqual(result, {'step': 'one'})

    def test_resume_unknown_head_refused_before_attachment(self):
        self.enable_fakes(); self.record.side_effect = [self.spec, {'unexpected': 'b' * 64}]
        with self.assertRaisesRegex(e.m.Refusal, 'four recovery history hashes'):
            e.run(self.path, self.sha, mode='resume-execute', heads_path=self.base / 'heads.json', heads_sha256='c'*64)
        self.host_type.from_verified_recovery.assert_not_called(); self.resume.assert_not_called()

    def test_inspection_never_constructs_host_or_executes(self):
        result = e.run(self.path, self.sha, mode='inspect')
        self.assertFalse(result['inspection']['evidence_valid'])
        self.inspector.assert_called_once(); self.host_type.assert_not_called()
        self.controller.assert_not_called(); self.resume.assert_not_called()


if __name__ == '__main__':
    unittest.main()
