"""Tiny policy tests: AST-extracted functions only; no production module imports."""
import ast
import hashlib
import json
from pathlib import Path
import plistlib
import shlex
import tempfile
from types import SimpleNamespace
import unittest

BASE = Path('/Users/v/other/astrid/.runtime/mlx-upgrade/reservoir-maintenance')
POLICY = BASE / 'evidence/provider-model-resolution-policy-01.json'
SERVER = Path('/Users/v/other/neural-triple-reservoir/coupled_astrid_server.py')
UTILS = SERVER.parent / '.venv/lib/python3.12/site-packages/mlx_lm/utils.py'
ENVIRONMENT = Path('/Users/v/other/astrid/scripts/model_environment.py')
PREFLIGHT = BASE / 'production-candidate-01/production_preflight.py'


def extracted(path, names, globals_):
    tree = ast.parse(path.read_text())
    nodes = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    if {node.name for node in nodes} != set(names):
        raise AssertionError('named source functions missing')
    future = ast.ImportFrom(module='__future__', names=[ast.alias(name='annotations')], level=0)
    module = ast.fix_missing_locations(ast.Module(body=[future, *nodes], type_ignores=[]))
    exec(compile(module, str(path) + ':AST-only', 'exec'), globals_)
    return globals_


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


class PolicyTests(unittest.TestCase):
    def setUp(self):
        self.policy = json.loads(POLICY.read_bytes())

    def test_policy_exact_sole_launch_argument_delta(self):
        old = self.policy['original_launch_argv']
        new = self.policy['provider_restart_argv']
        self.assertEqual(old.count('--model'), 1)
        self.assertEqual(len(old), len(new))
        self.assertEqual([i for i, pair in enumerate(zip(old, new)) if pair[0] != pair[1]], [old.index('--model') + 1])
        self.assertEqual(new[new.index('--model') + 1], self.policy['snapshot'])
        original = plistlib.loads(Path('/Users/v/Library/LaunchAgents/com.reservoir.coupled-astrid.plist').read_bytes())
        self.assertEqual(original['ProgramArguments'], old)

    def test_existing_local_loader_never_downloads(self):
        calls = []
        def forbidden(*args, **kwargs):
            calls.append((args, kwargs))
            raise AssertionError('Hub access forbidden in AST fixture')
        ns = extracted(UTILS, {'_download'}, {'Path': Path, 'snapshot_download': forbidden})
        with tempfile.TemporaryDirectory(prefix='model-policy-') as temporary:
            self.assertEqual(ns['_download'](temporary), Path(temporary))
        self.assertEqual(calls, [])

    def test_local_unified_config_preserves_shim_without_hub_import(self):
        calls = []
        original_import = __import__
        def restricted_import(name, *args, **kwargs):
            if name == '__future__':
                return original_import(name, *args, **kwargs)
            raise AssertionError('No imports allowed in extracted local config function: ' + name)
        ns = extracted(SERVER, {'_gemma4_unified_config_override'}, {
            'Path': Path, 'json': json,
            'log': SimpleNamespace(info=lambda *args: None),
            '_install_gemma4_unified_sanitize_patch': lambda: calls.append('shim'),
            '__builtins__': dict(vars(__import__('builtins')), __import__=restricted_import),
        })
        with tempfile.TemporaryDirectory(prefix='model-policy-') as temporary:
            config = Path(temporary) / 'config.json'
            config.write_text('{"model_type":"gemma4_unified"}')
            self.assertEqual(ns['_gemma4_unified_config_override'](temporary), {'model_type': 'gemma4'})
            config.write_text('{"model_type":"gemma4"}')
            self.assertIsNone(ns['_gemma4_unified_config_override'](temporary))
        self.assertEqual(calls, ['shim'])

    def test_framework_process_and_launch_alias_require_identical_tail(self):
        ns = extracted(ENVIRONMENT, {'interpreter_paths', 'command_matches'}, {'Path': Path})
        line = self.policy['original_process_identity_line']
        process = shlex.split(line.split('2026', 1)[1].strip())
        launch = self.policy['original_launch_argv']
        self.assertNotEqual(process[0], launch[0])
        self.assertTrue(ns['command_matches'](process, launch))
        pinned = list(process)
        pinned[pinned.index('--model') + 1] = self.policy['snapshot']
        self.assertTrue(ns['command_matches'](pinned, self.policy['provider_restart_argv']))
        self.assertFalse(ns['command_matches'](pinned, launch))
        foreign = list(process); foreign[0] = '/tmp/unowned-python'
        self.assertFalse(ns['command_matches'](foreign, launch))

    def setup_restart(self, temporary):
        original = {'ProgramArguments': self.policy['original_launch_argv'], 'WorkingDirectory': str(SERVER.parent),
                    'KeepAlive': True, 'EnvironmentVariables': {'PRESERVE': 'yes'}}
        origin = Path(temporary) / 'original.plist'
        origin.write_bytes(plistlib.dumps(original))
        restart = Path(temporary) / 'restart.plist'
        restart.write_bytes(plistlib.dumps(dict(original, ProgramArguments=self.policy['provider_restart_argv'])))
        ns = extracted(PREFLIGHT, {'pinned_argv', 'validate_provider_restart'}, {
            'm': SimpleNamespace(require=require), 'Path': Path, 'plistlib': plistlib, 'sha': sha,
            'PLISTS': {'provider': origin},
        })
        spec = {'plan': {'roles': {'provider': {'identity': {'argv': self.policy['original_launch_argv']}}}},
                'provider_restart_argv': list(self.policy['provider_restart_argv']),
                'provider_restart_plist': {'path': str(restart), 'sha256': sha(restart)}}
        return ns, spec, origin, restart

    def test_preflight_preserves_original_and_accepts_exact_pin(self):
        with tempfile.TemporaryDirectory(prefix='model-policy-') as temporary:
            ns, spec, origin, restart = self.setup_restart(temporary)
            before = origin.read_bytes()
            ns['validate_provider_restart'](spec, self.policy['snapshot'])
            self.assertEqual(origin.read_bytes(), before)

    def test_preflight_rejects_missing_duplicate_model_and_wrong_snapshot(self):
        with tempfile.TemporaryDirectory(prefix='model-policy-') as temporary:
            ns, spec, _, _ = self.setup_restart(temporary)
            for argv in [[], ['python', '--model'], ['python', '--model', 'a', '--model', 'b']]:
                with self.assertRaises(ValueError):
                    ns['pinned_argv'](argv, self.policy['snapshot'])
            for path in ['/tmp/other-model', self.policy['snapshot'] + '-different']:
                spec['provider_restart_argv'][8] = path
                with self.assertRaises(ValueError):
                    ns['validate_provider_restart'](spec, self.policy['snapshot'])

    def test_preflight_rejects_unrelated_arguments_and_plist_changes(self):
        with tempfile.TemporaryDirectory(prefix='model-policy-') as temporary:
            ns, spec, _, restart = self.setup_restart(temporary)
            spec['provider_restart_argv'][3] = '8092'
            with self.assertRaises(ValueError):
                ns['validate_provider_restart'](spec, self.policy['snapshot'])
            spec['provider_restart_argv'] = self.policy['provider_restart_argv']
            changed = plistlib.loads(restart.read_bytes()); changed['KeepAlive'] = False
            restart.write_bytes(plistlib.dumps(changed)); spec['provider_restart_plist']['sha256'] = sha(restart)
            with self.assertRaises(ValueError):
                ns['validate_provider_restart'](spec, self.policy['snapshot'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
