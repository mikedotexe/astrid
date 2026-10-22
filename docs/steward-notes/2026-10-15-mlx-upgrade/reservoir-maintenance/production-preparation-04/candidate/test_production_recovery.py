"""Only synthetic retained files and in-memory reports; no host observations."""
import ast
import copy
from pathlib import Path
import unittest

import production_recovery as r
import production_recovery_inspect as inspector
import test_production_recovery_inspect as fixtures


class PlanningTests(unittest.TestCase):
    def setUp(self):
        self.f = fixtures.RecoveryTests(); self.f.setUp(); self.addCleanup(self.f.doCleanups)
        self.f.event('preflight', receipt={'synthetic_only': True})

    def report(self):
        result = self.f.report(); self.f.valid(result); return result

    def state(self):
        return r.reconstruct(self.f.spec, self.report())

    def held(self, role='provider'):
        f = self.f; old = f.plan['roles'][role]['identity']
        f.hold(role); f.intent('arm_next_spawn', role)
        argv = ['/usr/bin/sudo', '--', '/bin/launchctl', 'debug', f'gui/{f.plan["host_binding"]["uid"]}/{f.plan["roles"][role]["label"]}',
                '--program', inspector.HOLDER_PYTHON, '--',
                str(f.tx/'model_environment.py'), 'launch', '--control-root', str(f.tx/('hold-'+role)), '--hold-only', f.plan['transaction_id']]
        f.host('debug_intent', role=role, command=argv, identity=old)
        f.host('debug_result', role=role, returncode=0); f.done('arm_next_spawn', role, {'armed': True})
        f.intent('signal_stop', role); f.host('signal_intent', role=role, identity=old, signal='SIGTERM', wall_time_ns=123456789)
        f.host('signal_sent', role=role, identity=old); f.done('signal_stop', role, {'signal': 'SIGTERM', 'identity': old, 'force_used': False})
        process = {'pid': 500 if role == 'provider' else 501, 'started_at': role+'-held',
                   'argv': [argv[6], *argv[8:]]}
        f.io.roles[role].update(process=process, listener_pids=[]); f.io.processes.pop(old['pid'])
        f.write(f.tx/('hold-'+role)/'held'/f'{process["pid"]}.json', {'schema': 'model_environment_held_v1',
                'pid': process['pid'], 'started_at': process['started_at'], 'hold': f.guard(), 'hold_only': True,
                'model_imported': False, 'listener_opened': False, 'launcher_sha256': f.sha(f.tx/'model_environment.py')})
        f.event('held_verified', role=role, identity=process)
        return process

    def witness(self, kind, **extra):
        f = self.f
        receipt = {'schema': 'reservoir_maintenance_witness_v1', 'kind': kind, 'passed': True,
                   'transaction_id': f.plan['transaction_id'], 'evidence': {'synthetic_only': True},
                   'bindings': {'plan_sha256': r.digest(f.plan), **{
                       role+'_old_identity_sha256': r.digest(f.plan['roles'][role]['identity']) for role in r.ROLES}, **extra}}
        f.write(f.tx/(kind+'-witness.json'), receipt); f.event('witness', kind=kind, receipt=receipt)
        return receipt

    def park(self, role):
        f = self.f; process = f.io.roles[role]['process']
        f.intent('remove_held', role); f.host('held_bootout_intent', role=role, identity=process)
        f.host('held_job_absent', role=role, identity=process); f.done('remove_held', role, {'removed_identity': process})
        f.event('job_absent', role=role)
        f.io.roles[role].update(job_loaded=False, process=None, listener_pids=[])

    def reservoir_held(self):
        self.held(); self.witness('provider_drain'); self.park('provider'); return self.held('reservoir')

    def assert_blocked(self, report):
        try:
            state = r.reconstruct(self.f.spec, report)
        except r.Refusal:
            return
        with self.assertRaises(r.Refusal): r.plan_resume(state)

    def test_provider_holder_preserves_exact_counters_guards_and_offsets(self):
        self.held(); report = self.report(); state = r.reconstruct(self.f.spec, report)
        self.assertEqual(state['blocked_reasons'], [])
        self.assertEqual(state['phase'], 'provider_held')
        self.assertEqual(state['host_state']['counter'], len(self.f.hosts))
        self.assertEqual(state['host_state']['anchors'], report['capture_anchors'])
        self.assertEqual(state['host_state']['guards']['provider'], self.f.guard())
        self.assertEqual(state['host_state']['signal_times'], {'provider': 123456789})
        self.assertEqual(state['host_state']['arm_attempts'], ['provider'])
        self.assertEqual(state['host_state']['signalled_identities']['provider'], self.f.plan['roles']['provider']['identity'])
        proposal = r.plan_resume(state)
        self.assertEqual(proposal['operation'], 'revalidate_provider_drain')
        self.assertFalse(proposal['execution_available']); self.assertFalse(proposal['retry_authorized'])

    def test_verified_checkin_is_required_before_holder_removal_proposal(self):
        self.held(); receipt = self.witness('provider_drain'); state = self.state()
        self.assertEqual(state['controller_state']['receipts']['provider_drain'], receipt)
        self.assertEqual(r.plan_resume(state)['operation'], 'revalidate_then_remove_verified_provider_holder')

    def test_reservoir_holder_requires_provider_park_and_retained_checkin(self):
        self.reservoir_held(); state = self.state()
        self.assertEqual(state['controller_state']['parked'], ['provider'])
        self.assertEqual(r.plan_resume(state)['operation'], 'revalidate_final_snapshot')
        self.witness('final_snapshot')
        self.assertEqual(r.plan_resume(self.state())['operation'], 'revalidate_then_remove_verified_reservoir_holder')

    def test_both_parked_only_proposes_backup_revalidation(self):
        self.reservoir_held(); final = self.witness('final_snapshot'); self.park('reservoir')
        backup = self.witness('backup', final_snapshot_sha256=r.digest(final))
        state = self.state(); proposal = r.plan_resume(state)
        self.assertEqual(state['controller_state']['parked'], ['provider', 'reservoir'])
        self.assertEqual(state['host_state']['receipts']['backup'], backup)
        self.assertEqual(proposal['operation'], 'revalidate_stopped_inventory_and_backup')
        self.assertNotIn('execute', proposal['operation'])

    def test_before_hold_and_before_verified_holder_are_unsupported(self):
        self.assert_blocked(self.report())
        self.f.signal(); self.f.holder()
        self.assert_blocked(self.report())

    def test_each_remove_durable_boundary_blocks_until_complete(self):
        self.held(); self.witness('provider_drain'); f = self.f; held = f.io.roles['provider']['process']
        f.intent('remove_held', 'provider'); self.assert_blocked(self.report())
        f.host('held_bootout_intent', role='provider', identity=held); self.assert_blocked(self.report())
        f.io.roles['provider'].update(job_loaded=False, process=None, listener_pids=[])
        self.assert_blocked(self.report())
        f.host('held_job_absent', role='provider', identity=held); self.assert_blocked(self.report())
        f.done('remove_held', 'provider', {'removed_identity': held}); self.assert_blocked(self.report())
        f.event('job_absent', role='provider'); self.assert_blocked(self.report())  # reservoir still running: unsupported next signal

    def test_unknown_debug_result_and_claimed_auth_refusal_never_rearm(self):
        self.f.debug(result=False, complete=False); self.assert_blocked(self.report())
        self.f.host('debug_result', role='provider', returncode=1, pre_exec_auth_refused=True,
                    stderr='sudo: a password is required\n')
        state = self.state()
        self.assertTrue(any('not proven pre-command' in x for x in state['blocked_reasons']))
        with self.assertRaises(r.Refusal): r.plan_resume(state)

    def test_signal_intent_and_host_result_without_controller_completion_block(self):
        self.f.signal(result=False, complete=False); self.assert_blocked(self.report())
        self.f.host('signal_sent', role='provider', identity=self.f.plan['roles']['provider']['identity'])
        self.assert_blocked(self.report())

    def test_terminal_failure_keeps_known_holder_but_no_automatic_authority(self):
        self.held(); self.f.event('failed', error_type='Interrupted', error='owned test', automatic_action='none')
        state = self.state(); self.assertEqual(len(state['controller_state']['failures']), 1)
        self.assertFalse(r.plan_resume(state)['execution_available'])

    def test_altered_controller_chain_and_reordered_prefix_refuse(self):
        self.held(); report = self.report()
        report['controller']['events'][2]['operation'] = 'arm_next_spawn'
        self.assert_blocked(report)
        report = self.report(); events = report['controller']['events']; events[1]['operation'] = 'signal_stop'
        prior = r.digest(self.f.plan)
        for event in events:
            event['previous_sha256'] = prior; prior = r.digest(event)
        report['controller']['last_sha256'] = prior
        self.assert_blocked(report)

    def test_absent_job_does_not_resolve_unrecorded_removal(self):
        self.held(); report = self.report(); row = report['roles']['provider']
        row['classification'] = 'absent_observed'; row['observed'].update(process=None, job_loaded=False)
        report['observed_before']['roles']['provider'] = copy.deepcopy(row['observed'])
        report['observed_after'] = copy.deepcopy(report['observed_before'])
        self.assert_blocked(report)

    def test_foreign_holder_and_pid_reuse_refuse(self):
        self.held()
        for mutation in ('identity', 'guard', 'reuse', 'listener'):
            with self.subTest(mutation=mutation):
                report = self.report(); row = report['roles']['provider']
                if mutation == 'identity': row['observed']['process']['started_at'] = 'reused'
                elif mutation == 'guard': row['guard']['actor'] = 'foreign'
                elif mutation == 'reuse': row['old_pid_observation'] = {'pid': 100, 'started_at': 'reused', 'argv': ['foreign']}
                else: row['all_tcp_listener_pids'] = [500]
                self.assert_blocked(report)

    def test_no_original_anchor_reset_or_proposal_reuse(self):
        self.held(); report = self.report(); state = r.reconstruct(self.f.spec, report); proposal = r.plan_resume(state)
        r.validate_proposal(proposal, self.f.spec, report)
        with self.assertRaisesRegex(r.Refusal, 'consumed'):
            r.validate_proposal(proposal, self.f.spec, report, consumed_tokens=[proposal['single_use_token']])
        changed = copy.deepcopy(report); changed['capture_anchors']['provider']['log']['start_offset'] += 1
        with self.assertRaisesRegex(r.Refusal, 'stale'):
            r.validate_proposal(proposal, self.f.spec, changed)
        self.assertEqual(state['host_state']['anchors']['provider']['log']['start_offset'], 0)

    def test_changed_artifact_or_host_operation_invalidates_proposal(self):
        self.held(); report = self.report(); proposal = r.plan_resume(r.reconstruct(self.f.spec, report))
        changed = copy.deepcopy(report); changed['artifacts']['synthetic-new'] = {'sha256': 'a'*64}
        with self.assertRaisesRegex(r.Refusal, 'stale'): r.validate_proposal(proposal, self.f.spec, changed)
        changed = copy.deepcopy(report); changed['host_operations']['events'][0]['monotonic'] += .01
        with self.assertRaisesRegex(r.Refusal, 'stale'): r.validate_proposal(proposal, self.f.spec, changed)

    def test_missing_or_started_restore_inspection_always_blocks(self):
        self.reservoir_held(); self.witness('final_snapshot'); self.park('reservoir')
        for status in (None, 'request_only', 'unresolved', 'exited_without_outer_witness', 'outer_witness_recorded'):
            with self.subTest(status=status):
                report = self.report()
                if status is None: report.pop('private_restore_execution')
                else: report['private_restore_execution'] = {'status': status}
                self.assert_blocked(report)

    def test_changed_source_or_pinned_plist_is_not_inferred_completed(self):
        self.held()
        for role, field, value in (('reservoir', 'source', 'candidate'), ('provider', 'plist', 'pinned')):
            report = self.report(); report['installed_variants'][role][field] = value
            self.assert_blocked(report)

    def test_protected_stewardship_and_observation_change_block(self):
        self.held()
        for field in ('protected', 'stewardship', 'host'):
            report = self.report(); report['observed_before'][field] = {}; report['observed_after'] = copy.deepcopy(report['observed_before'])
            self.assert_blocked(report)

    def test_no_input_mutation_and_no_io_or_execution_surface(self):
        self.held(); report = self.report(); saved = copy.deepcopy((self.f.spec, report))
        r.plan_resume(r.reconstruct(self.f.spec, report)); self.assertEqual(saved, (self.f.spec, report))
        tree = ast.parse(Path(r.__file__).read_text())
        imports = {n.names[0].name for n in ast.walk(tree) if isinstance(n, ast.Import)}
        self.assertEqual(imports, {'copy', 'hashlib', 'json'})
        forbidden = {'open', 'read_text', 'read_bytes', 'write_text', 'write_bytes', 'Popen', 'run', 'kill', 'unlink', 'main'}
        names = {n.name for n in ast.walk(tree) if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        calls = {n.func.id if isinstance(n.func, ast.Name) else n.func.attr for n in ast.walk(tree)
                 if isinstance(n, ast.Call) and isinstance(n.func, (ast.Name, ast.Attribute))}
        self.assertFalse((names | calls) & forbidden)

    def test_schedule_matches_actual_controller03_success_with_inert_host(self):
        import test_production_controller as controller_fixtures
        fixture = controller_fixtures.ProductionControllerTests()
        fixture.setUp()
        try:
            fixture.run_enabled_fake()  # Inert Host; the real production flag is restored.
            events = fixture.journal.events
            record = {'events': events, 'last_sha256': r.digest(events[-1]), 'pending': None}
            state = r.controller_state(fixture.plan, record)
            self.assertEqual(state['prefix_length'], len(r.schedule()))
            self.assertTrue(state['repaired']); self.assertTrue(state['provider_pinned'])
            self.assertEqual(set(state['replacements']), set(r.ROLES))
            self.assertEqual(state['parked'], set())
        finally:
            fixture.doCleanups()


if __name__ == '__main__': unittest.main(verbosity=2)
