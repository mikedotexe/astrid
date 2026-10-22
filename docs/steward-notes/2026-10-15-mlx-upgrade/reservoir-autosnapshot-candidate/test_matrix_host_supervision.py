import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('matrix_host_supervision', Path(__file__).with_name('matrix_host_supervision.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def sample(stamp=100):
    return {'t_s': stamp, 'pressure_level': 1, 'swap_bytes': 1000,
            'build_test_processes': [], 'production_identity_unchanged': True,
            'readyz': {'http_status': 200, 'body': {'ready': True, 'reservoir': {'status': 'connected'}}}}


def record():
    return {'state': 'running', 'invalidated': False, 'initial_swap_bytes': 1000,
            'samples': [sample(100), sample(105)], 'identity_start': {'pid': 7}}


class EvidenceTests(unittest.TestCase):
    def test_case_requires_bracketing_samples(self):
        r = record()
        self.assertTrue(m.covers_case(r, 101, 104))
        self.assertFalse(m.covers_case(r, 99, 104))
        self.assertFalse(m.covers_case(r, 101, 106))
        self.assertFalse(m.covers_case(r, 104, 101))

    def test_next_release_requires_fresh_running_evidence(self):
        r = record()
        self.assertTrue(m.ready_for_next_case(r, 106))
        self.assertFalse(m.ready_for_next_case(r, 121))
        self.assertFalse(m.ready_for_next_case(r, 104))
        r['state'] = 'completed'
        self.assertFalse(m.ready_for_next_case(r, 106))
        self.assertTrue(m.covers_case(r, 101, 104))

    def test_no_or_single_sample_never_releases_case(self):
        for rows in ([], [sample()]):
            r = record(); r['samples'] = rows
            self.assertFalse(m.ready_for_next_case(r, 106))
            self.assertFalse(m.covers_case(r, 101, 104))

    def test_bad_host_observations_fail_closed(self):
        variants = [{'pressure_level': 2}, {'swap_bytes': 1001}, {'swap_bytes': float('nan')},
                    {'build_test_processes': [123]}, {'build_test_processes': None},
                    {'production_identity_unchanged': False}, {'readyz': {}},
                    {'memory_error': 'unknown'}, {'process_error': 'unknown'}]
        for change in variants:
            with self.subTest(change=change):
                row = sample(105); row.update(change)
                self.assertTrue(m.violations(row, initial_swap=1000, previous_t=100))

    def test_invalid_time_gap_and_initial_swap_refuse(self):
        for stamp in (99, 100, 115.001, float('nan')):
            self.assertTrue(m.violations(sample(stamp), initial_swap=1000, previous_t=100))
        self.assertFalse(m.violations(sample(115), initial_swap=1000, previous_t=100))
        self.assertTrue(m.violations(sample(), initial_swap=float('nan'), previous_t=None))

    def test_invalidation_cannot_be_cleared_by_later_good_sample(self):
        r = record(); r['invalidated'] = True
        m.observe({'host_sample': lambda: sample(110), 'identities': lambda: {'pid': 7}}, r)
        self.assertTrue(r['invalidated'])
        self.assertFalse(m.covers_case(r, 101, 109))


class StopTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for name, value in [('ROOT', self.root), ('OUTPUT', self.root/'out.json'), ('STOP', self.root/'stop.json')]:
            p = patch.object(m, name, value); p.start(); self.addCleanup(p.stop)

    def stop(self):
        m.STOP.write_text(json.dumps({'schema': 'reservoir_matrix_host_stop_v1', 'token': m.TOKEN, 'operation': 'finish'}))

    def test_valid_owned_finish(self):
        self.stop(); m.read_stop()

    def test_symlink_and_oversized_stop_refuse(self):
        target = self.root/'target'; target.write_text('{}'); m.STOP.symlink_to(target)
        with self.assertRaises(RuntimeError): m.read_stop()
        m.STOP.unlink(); m.STOP.write_text('x'*513)
        with self.assertRaises(RuntimeError): m.read_stop()

    def test_foreign_owner_and_writable_stop_refuse(self):
        self.stop()
        uid = os.getuid()
        with patch.object(m.os, 'getuid', return_value=uid+1):
            with self.assertRaises(RuntimeError): m.read_stop()
        m.STOP.chmod(0o666)
        with self.assertRaises(RuntimeError): m.read_stop()

    def test_immediate_stop_after_identity_refuses_zero_health(self):
        def identity(): self.stop(); return {'pid': 7}
        with patch.object(m, 'read_functions', return_value={'identities': identity}):
            with self.assertRaisesRegex(RuntimeError, 'before any health'): m.main()
        out = json.loads(m.OUTPUT.read_text())
        self.assertTrue(out['invalidated']); self.assertEqual(out['state'], 'failed')

    def test_finish_collects_final_health_and_retains_failure(self):
        calls = []
        def host_sample():
            calls.append(1)
            if len(calls) > 2: raise AssertionError('unexpected extra observation')
            row = sample(100 if len(calls) == 1 else 105)
            if len(calls) == 1: self.stop()
            else: row['pressure_level'] = 2
            return row
        with patch.object(m, 'read_functions', return_value={'identities': lambda: {'pid': 7}, 'host_sample': host_sample}), patch.object(m.time, 'sleep'):
            self.assertEqual(m.main(), 1)
        out = json.loads(m.OUTPUT.read_text())
        self.assertEqual(len(out['samples']), 2)
        self.assertTrue(out['invalidated']); self.assertEqual(out['completion_reason'], 'owned_finish_request')


if __name__ == '__main__': unittest.main()
