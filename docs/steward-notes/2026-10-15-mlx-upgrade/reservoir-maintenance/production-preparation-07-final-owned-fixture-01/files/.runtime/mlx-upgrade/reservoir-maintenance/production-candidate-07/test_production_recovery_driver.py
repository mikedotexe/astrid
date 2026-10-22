"""Finite recovery effects with inert Host fakes and private JSON files only.

Real retained Host/inspector pairing is covered separately by test_production_retained.
These cases exercise every driver phase, failure boundary, and at-most-once claim.
"""
import copy
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import reservoir_maintenance as m
import production_recovery_driver as d
import recovery_phases as phases
from recovery_ledger import read_ledger, RecoveryLedger
from transaction_execution import require_active
from test_production_controller import ProductionFake, legacy


class RecoveryFake(ProductionFake):
    def __init__(self, root, plan, scope='production'):
        super().__init__(plan); self.root = root; self.scope = scope
        self.spec = {'plan': copy.deepcopy(plan), 'transaction_root': str(root),
                     'execution_scope': scope, 'provider_restart_argv': self.proof['provider_restart_argv']}
        self.proof['preparation_sha256'] = m.digest(self.spec)
        self.saved = {}; self.validated = None; self.reconciliations = []; self.after_revalidate = None
        self.confirmed_armed = set()
        self.anchor = {r: {'identity': copy.deepcopy(plan['roles'][r]['identity']),
                           'log': {'offset': 9731, 'prefix_sha256': m.digest(r)},
                           'qos': {'offset': 761}, 'journal': {'sha256': m.digest('original-journal')}} for r in m.ROLES}
        if scope == 'owned_fixture':
            for r in m.ROLES: self.roles[r]['target'] = f"gui/501/{m.FIXTURE_PREFIX}{plan['transaction_id']}.{r}"

    def event(self, op, role=None):
        if op != 'admission':
            require_active(self.root, self.plan)
            count = len(list(self.root.glob('host-operation-*.json')))
            m.write_new(self.root / f'host-operation-{count+1:05d}.json',
                        {'operation': 'inert_fake_'+op, 'role': role, 'ordinal': count+1})
        return super().event(op, role)

    def recovery_inspection(self, journal):
        self.clock += 1  # Advance only fake time; no wait or live observation.
        view = {}; ledger = read_ledger(self.root, self.plan)
        for role in m.ROLES:
            observed = self.inspect_role(role); process = observed['process']; old = self.plan['roles'][role]['identity']
            classification = ('original_identity_observed' if process == old else
                              'owned_holder_observed' if observed['held'] else
                              'absent_observed' if process is None else 'recorded_replacement_observed')
            view[role] = {'guard': copy.deepcopy(self.holds.get(role)), 'observed': observed,
                          'old_pid_observation': old if process == old else None, 'classification': classification}
        observed = {'host': self.plan['host_binding'], 'protected': self.protected,
                    'stewardship': {'paused':True,'pause_generation':self.pause_generation,'pause_actor':self.plan['actor'],'lease':None}}
        return {'evidence_valid':True, 'errors':[], 'transaction_id':self.plan['transaction_id'],
                'preparation_sha256':m.digest(self.spec), 'observed_before':copy.deepcopy(observed),
                'observed_after':copy.deepcopy(observed), 'capture_anchors':copy.deepcopy(self.anchor),
                'controller':{'events':copy.deepcopy(journal.events), 'last_sha256':journal.previous},
                'host_operations': {'events':d.host_history(self.root)['events']}, 'signal_wall_time_ns':{},
                'roles':view, 'unresolved':[], 'recovery_ledger':ledger,
                'recovery_output_presence':{kind:True for kind in self.saved},
                'installed_variants':{
                    'provider':{'source':'original','plist':'pinned' if self.applied else 'original'},
                    'reservoir':{'source':'candidate' if self.inputs['reservoir_source']==self.inputs['reservoir_candidate'] else 'original','plist':'original'}}}

    def revalidate_recovery(self, cp, report):
        require_active(self.root, self.plan)
        actual = phases.checkpoint_from_inspection(self.spec, report, read_ledger(self.root, self.plan))
        m.require(actual == cp, 'fake rejected unverified checkpoint')
        self.validated = copy.deepcopy(cp)
        if self.after_revalidate: self.after_revalidate()
        return actual

    def collect_witness(self, kind, context):
        m.require(kind not in self.saved, 'cannot replay witness producer')
        result = super().collect_witness(kind, context); self.saved[kind] = copy.deepcopy(result)
        m.write_new(self.root / (kind+'-witness.json'), result)
        return result

    def apply_provider_restart(self, ctx):
        return super().apply_provider_restart(ctx)

    def arm_next_spawn(self, role, guard, expected):
        result=super().arm_next_spawn(role,guard,expected)
        self.confirmed_armed.add(role)  # Inert analogue of a positive retained Host result.
        return result

    def bootstrap_original(self, role, ctx):
        if self.scope == 'owned_fixture': return legacy.FakeHost.bootstrap_original(self, role, ctx)
        return super().bootstrap_original(role, ctx)

    def reconcile_recovery(self, action_id, cp, report):
        require_active(self.root, self.plan); m.require(self.validated == cp, 'unvalidated reconciliation')
        action=cp['action']; op=action['operation']; role=action['role']; self.reconciliations.append(action_id)
        view=self.inspect_role(role) if role else None
        if action['kind'] == 'preflight': result=m.preflight(self.plan,self)
        elif action['kind'] == 'completed': result=d.execute_one(d.hydrate_controller(m.Journal(self.root/'controller'),self,cp),cp,action)
        elif action['kind'] == 'witness':
            m.require(action['witness'] in self.saved, 'incomplete witness; no producer retry')
            result=copy.deepcopy(self.saved[action['witness']])
        elif op=='create_hold':
            m.require(self.holds.get(role)==cp['guard'],'guard effect absent'); result={'guard':cp['guard']}
        elif op=='arm_next_spawn' and role in self.confirmed_armed and view['process']==self.plan['roles'][role]['identity']:
            result={'armed':True}
        elif op in ('arm_next_spawn','signal_stop','held_verified'):
            m.require(view['held'] is not None and view['held']['guard']==cp['guard'],'unknown debug/signal; original remains; manual stop')
            result=({'identity':view['process']} if op=='held_verified' else
                    {'effect':'owned_holder_observed','identity':view['process'],
                     'original_identity':self.plan['roles'][role]['identity'],'guard':cp['guard']})
        elif op in ('remove_held','job_absent'):
            m.require(not view['job_loaded'] and view['process'] is None and view['listener_pids']==[],'role not absent')
            result={'removed_identity':cp['holders'][role]} if op=='remove_held' else {'absent':True}
        elif op=='install_repair':
            m.require(self.inputs['reservoir_source']==self.inputs['reservoir_candidate'],'install effect absent')
            result={'source_sha256':self.inputs['reservoir_source']}
        elif op=='apply_provider_restart':
            m.require(self.applied,'pin effect absent'); result={'provider_settings_sha256':self.inputs['provider_settings'],
                'provider_restart_argv':self.proof['provider_restart_argv']}
        elif op=='bootstrap_original':
            expected=self.spec['provider_restart_argv'] if role=='provider' and self.scope=='production' else self.plan['roles'][role]['identity']['argv']
            m.require(view['process'] is not None and view['process']!=self.plan['roles'][role]['identity']
                      and view['process']['argv']==expected and view['listener_pids']==[view['process']['pid']], 'replacement effect absent')
            result={'identity':view['process']}
        elif op=='release_hold':
            m.require(role not in self.holds,'release effect absent'); result={'released':True}
        else: raise m.Refusal('unsupported fake observation')
        return {'schema':'reservoir_recovery_reconciliation_v1','passed':True,'action_id':action_id,
                'transaction_id':self.plan['transaction_id'],'plan_sha256':m.digest(self.plan),
                'checkpoint_sha256':m.digest(cp),'inspection_sha256':m.digest(report),'result':result,
                'evidence':{'scope':'stdlib_fake_only','historical_return_inferred':False}}


class DriverTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup);self.root=Path(self.tmp.name).resolve()
        self.plan=legacy.fake_plan();self.plan['roles']['provider']['identity']['argv']+=['--model','hub/id']
        self.plan['input_hashes']['provider_restart_settings']=m.digest('pinned')
        self.host=RecoveryFake(self.root,self.plan);self.journal=m.Journal(self.root/'controller',plan=self.plan)
        self.journal.append('failed',error='inert interruption before first observation')
        self.original=copy.deepcopy(self.journal.events)

    def checkpoint(self):
        report=self.host.recovery_inspection(m.Journal(self.journal.root))
        return phases.checkpoint_from_inspection(self.host.spec,report,read_ledger(self.root,self.plan))

    def step(self, mode=None):
        cp=self.checkpoint()
        with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True):
            return d.resume_one(self.journal,self.host,expected=d.expected_heads(cp),
                                mode=mode or ('reconcile' if cp['requires_reconciliation'] else 'execute'))

    def advance(self, action_id):
        for _ in range(40):
            cp=self.checkpoint()
            if cp['action_id']==action_id:return cp
            self.assertFalse(cp['completed']);self.step()
        self.fail('finite step bound exceeded')

    def test_all_phases_one_step_each_preserve_original_anchors_and_journal(self):
        for _ in range(40):
            cp=self.checkpoint()
            if cp['completed']:break
            result=self.step();self.assertFalse(result['automatic_next_action'])
        else:self.fail('not completed')
        self.assertEqual(m.Journal(self.journal.root).events,self.original)
        self.assertEqual(cp['anchors'],self.host.anchor)
        self.assertTrue(cp['repaired']);self.assertTrue(cp['provider_pinned']);self.assertFalse(self.host.holds)
        self.assertEqual(cp['replacements']['provider']['argv'],self.host.proof['provider_restart_argv'])
        for name,role in [('signal_stop','provider'),('signal_stop','reservoir'),('bootstrap_original','provider'),('bootstrap_original','reservoir')]:
            self.assertEqual(self.host.calls.count((name,role)),1)
        self.assertEqual([r['action_id'] for r in read_ledger(self.root,self.plan)['records'] if r['type'] in ('action_result','reconciliation_result')],
                         [a['id'] for a in phases.actions()])

    def test_owned_fixture_uses_same_full_driver_without_production_flag(self):
        self.host=RecoveryFake(self.root,self.plan,scope='owned_fixture')
        for _ in range(40):
            cp=self.checkpoint()
            if cp['completed']:break
            d.resume_one(self.journal,self.host,expected=d.expected_heads(cp),mode='reconcile' if cp['requires_reconciliation'] else 'execute')
        self.assertTrue(cp['completed']);self.assertFalse(cp['provider_pinned'])
        self.assertNotIn(('apply_provider_restart','provider'),self.host.calls)

    def test_default_disabled_before_lock_host_or_ledger(self):
        self.assertIs(m.PRODUCTION_CONTROLLER_ENABLED,True)
        with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',False),self.assertRaisesRegex(m.Refusal,'disabled'):
            d.resume_one(self.journal,self.host,expected={k:'a'*64 for k in d.HEAD_KEYS})
        self.assertFalse((self.root/'execution.lock').exists());self.assertEqual(self.host.calls,[])

    def test_stale_heads_and_change_during_revalidate_no_action_claim(self):
        cp=self.checkpoint();bad=d.expected_heads(cp);bad['controller_head_sha256']='f'*64
        with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True),self.assertRaisesRegex(m.Refusal,'stale'):
            d.resume_one(self.journal,self.host,expected=bad)
        self.assertFalse((self.root/'recovery').exists())
        self.host.after_revalidate=lambda:self.journal.append('failed',error='external journal change')
        with self.assertRaisesRegex(m.Refusal,'history changed'):self.step()
        self.assertFalse((self.root/'recovery').exists())

    def test_claim_write_failure_has_no_action_and_next_explicit_attempt_allowed(self):
        append=RecoveryLedger.append
        def fail(log,kind,**details):
            if kind=='action_claim':raise OSError('claim write refused')
            return append(log,kind,**details)
        with patch.object(RecoveryLedger,'append',fail),self.assertRaises(OSError):self.step()
        self.assertEqual([call for call in self.host.calls if call[0]!='inspect'],[])
        self.assertEqual(read_ledger(self.root,self.plan)['records'],[])
        self.assertEqual(self.step()['action_id'],'preflight')

    def test_every_completed_effect_can_be_adopted_after_result_write_failure(self):
        # One failure at each phase in a single finite transaction. Normal action
        # effects and witness producers each happen once; reconciliation is read-only.
        append=RecoveryLedger.append
        for _ in range(40):
            cp=self.checkpoint()
            if cp['completed']:break
            action=cp['action_id'];mode='reconcile' if cp['requires_reconciliation'] else 'execute'
            def fail(log,kind,**details):
                if kind in ('action_result','reconciliation_result'):raise OSError('result write interrupted')
                return append(log,kind,**details)
            with patch.object(RecoveryLedger,'append',fail),self.assertRaisesRegex(OSError,'interrupted'):self.step(mode)
            before=list(self.host.calls)
            with self.assertRaises(m.Refusal):self.step('execute')
            self.assertEqual(self.host.calls,before)
            self.assertEqual(self.step('reconcile')['action_id'],action)
        self.assertTrue(self.checkpoint()['completed'])
        self.assertEqual(m.Journal(self.journal.root).events,self.original)
        for name in ('provider_drain','final_snapshot','backup','private_restore','startup','provider_ready'):
            self.assertEqual(self.host.calls.count(('witness',name)),1)

    def test_unknown_armed_outcome_while_original_alive_never_resignals(self):
        self.advance('arm_next_spawn:provider');self.host.fail=('arm_next_spawn','provider')
        with self.assertRaises(RuntimeError):self.step()
        before=list(self.host.calls)
        with self.assertRaisesRegex(m.Refusal,'never replay'):self.step('execute')
        with self.assertRaisesRegex(m.Refusal,'manual stop'):self.step('reconcile')
        self.assertEqual(before,self.host.calls);self.assertNotIn(('signal_stop','provider'),self.host.calls)

    def test_absent_effect_before_command_remains_blocked_without_retry(self):
        self.advance('create_hold:provider');self.host.fail=('create_hold','provider')
        with self.assertRaises(RuntimeError):self.step()
        with self.assertRaisesRegex(m.Refusal,'effect absent'):self.step('reconcile')
        with self.assertRaisesRegex(m.Refusal,'never replay'):self.step('execute')
        self.assertEqual(self.host.calls.count(('create_hold','provider')),1)

    def test_other_role_protected_or_source_drift_refuses_before_claim(self):
        self.advance('create_hold:provider')
        for field in ('process','protected','source'):
            roles=copy.deepcopy(self.host.roles);protected=copy.deepcopy(self.host.protected);inputs=copy.deepcopy(self.host.inputs)
            if field=='process':self.host.roles['reservoir']['process']['pid']+=999
            elif field=='protected':self.host.protected['bridge']['pid']+=999
            else:self.host.inputs['reservoir_source']=self.host.inputs['reservoir_candidate']
            count=len(read_ledger(self.root,self.plan)['records'])
            with self.subTest(field=field),self.assertRaises(m.Refusal):self.step()
            self.assertEqual(len(read_ledger(self.root,self.plan)['records']),count)
            self.host.roles=roles;self.host.protected=protected;self.host.inputs=inputs

    def test_partial_backup_or_restore_never_restarts_producer(self):
        self.advance('witness:backup');self.host.saved['backup']={'passed':False}
        with self.assertRaises(m.Refusal):self.step('reconcile')
        with self.assertRaises(m.Refusal):self.step('execute')
        self.assertNotIn(('witness','backup'),self.host.calls)
        self.assertNotIn(('install_repair',None),self.host.calls)

    def test_accepted_late_history_is_never_rolled_back(self):
        self.advance('witness:provider_ready');history=self.root/'authored-history';history.write_text('new output after restored provider admission')
        self.step();self.step();self.step();self.step()
        self.assertEqual(history.read_text(),'new output after restored provider admission')
        self.assertTrue(self.checkpoint()['completed'])

    def test_full_original_controller_schedule_matches_pure_reconstruction(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp).resolve();journal=m.Journal(root/'controller',plan=self.plan)
            host=RecoveryFake(root,self.plan)
            with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True):m.Controller(journal,host).run()
            report=host.recovery_inspection(journal)
            cp=phases.checkpoint_from_inspection(host.spec,report,read_ledger(root,self.plan))
            self.assertTrue(cp['completed']);self.assertEqual(cp['action_index'],len(phases.actions()))


if __name__=='__main__':unittest.main(verbosity=2)
