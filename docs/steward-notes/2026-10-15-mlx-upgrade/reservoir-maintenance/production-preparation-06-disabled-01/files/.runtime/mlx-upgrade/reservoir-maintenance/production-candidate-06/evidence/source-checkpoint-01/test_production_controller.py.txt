"""Inert state-machine fakes. No Host, process, service, socket, or numeric execution."""
import copy
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import reservoir_maintenance as m
HERE=Path(__file__).resolve().parent
LEGACY=HERE.parent/'candidate-v2/test_reservoir_maintenance.py'
spec=importlib.util.spec_from_file_location('controller_legacy_fakes',LEGACY)
legacy=importlib.util.module_from_spec(spec);spec.loader.exec_module(legacy)
LegacyFixtureTests=legacy.MaintenanceTests

def admission(plan):
    argv=list(plan['roles']['provider']['identity']['argv']);argv[argv.index('--model')+1]='/fake/snapshot'
    refs={name:{'path':'/fake/'+name+'.json','sha256':m.digest(name)} for name in
          ('execution_review','fixture','fixture_review','real_matrix','restore_qualification')}
    refs['fixture']['sha256']=plan['input_hashes']['fixture']
    return {'schema':'reservoir_production_admission_v1','validated':True,'plan_sha256':m.digest(plan),
            'preparation_sha256':'a'*64,'references':refs,'model_snapshot':'/fake/snapshot',
            'provider_restart_argv':argv,'reservoir_candidate_sha256':plan['input_hashes']['reservoir_candidate'],
            'provider_restart_settings_sha256':plan['input_hashes']['provider_restart_settings'],
            'integrated_artifacts':{'/fake/helper.py':'b'*64}}

class ProductionFake(legacy.FakeHost):
    scope='production'
    def __init__(self,plan):
        super().__init__(plan);self.proof=admission(plan);self.applied=False;self.bad_applied=False;self.change_early=False
        for role in m.ROLES:self.roles[role]['target']=f"gui/{plan['host_binding']['uid']}/{m.LABELS[role]}"
    def production_admission(self,plan):
        self.event('admission');assert plan==self.plan;return copy.deepcopy(self.proof)
    def executor_identity(self):
        return {'pid':os.getpid(),'started_at':'inert-test-executor','argv':['inert-test-controller']}
    def apply_provider_restart(self,context):
        self.event('apply_provider_restart','provider')
        assert not self.roles['provider']['job_loaded'] and 'startup' in self.witnesses
        self.inputs['provider_settings']=self.inputs['provider_restart_settings'];self.applied=True
        return {'provider_settings_sha256':self.inputs['provider_settings'] if not self.bad_applied else 'f'*64,
                'provider_restart_argv':self.proof['provider_restart_argv']}
    def bootstrap_original(self,role,context):
        if role=='provider':assert self.applied
        value=super().bootstrap_original(role,context)
        if role=='provider':
            value['identity']['argv']=copy.deepcopy(self.proof['provider_restart_argv'])
            self.roles[role]['process']=copy.deepcopy(value['identity'])
        return value
    def install_repair(self,context):
        result=super().install_repair(context)
        if self.change_early:self.inputs['provider_settings']=self.inputs['provider_restart_settings']
        return result

class ProductionControllerTests(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.plan=legacy.fake_plan();self.plan['roles']['provider']['identity']['argv']+=['--model','hub/id','--port','8090']
        self.plan['input_hashes']['provider_restart_settings']=m.digest('restart-settings');self.plan=m.validate_plan(self.plan)
        self.host=ProductionFake(self.plan);self.journal=m.Journal(Path(self.tmp.name).resolve()/'controller',plan=self.plan)
        self.controller=m.Controller(self.journal,self.host)
    def run_enabled_fake(self):
        with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True):return self.controller.run()
    def test_default_refuses_before_any_host_call(self):
        self.assertIs(m.PRODUCTION_CONTROLLER_ENABLED,False)
        with self.assertRaisesRegex(m.Refusal,'disabled'):self.controller.run()
        self.assertEqual(self.host.calls,[]);self.assertEqual(self.journal.events,[])
    def test_exact_production_sequence_and_original_drain_identity(self):
        original=copy.deepcopy(self.plan['roles']['provider']['identity']);result=self.run_enabled_fake()
        self.assertEqual(result['provider']['argv'],self.host.proof['provider_restart_argv'])
        self.assertEqual(self.plan['roles']['provider']['identity'],original);calls=self.host.calls
        self.assertLess(calls.index(('witness','startup')),calls.index(('apply_provider_restart','provider')))
        self.assertLess(calls.index(('apply_provider_restart','provider')),calls.index(('bootstrap_original','provider')))
        applied=next(i for i,e in enumerate(self.journal.events) if e.get('operation')=='apply_provider_restart' and e['type']=='mutation_completed')
        boot=next(i for i,e in enumerate(self.journal.events) if e.get('operation')=='bootstrap_original' and e.get('role')=='provider' and e['type']=='mutation_intent')
        self.assertLess(applied,boot);self.assertTrue(self.controller.provider_pinned);self.assertFalse(self.host.holds)
    def test_unreviewed_or_incomplete_admission_refused(self):
        for key in ('plan_sha256','preparation_sha256','references','integrated_artifacts'):
            value=admission(self.plan);value.pop(key)
            with self.subTest(key=key),self.assertRaises(m.Refusal):m.validate_production_admission(self.plan,value)
    def test_model_substitution_cannot_change_other_arguments(self):
        value=admission(self.plan);value['provider_restart_argv']+=['--temperature','1']
        with self.assertRaisesRegex(m.Refusal,'unrelated arguments'):m.validate_production_admission(self.plan,value)
    def test_target_never_broadens_to_foreign_label_or_uid(self):
        for target in ('gui/502/'+m.LABELS['provider'],'gui/501/foreign','fake:provider'):
            view=self.host.inspect_role('provider');view['target']=target
            with self.subTest(target=target),patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True),self.assertRaises(m.Refusal):m.check_target(self.host,'provider',view,self.plan)
    def test_early_provider_plist_change_refused_before_bootstrap(self):
        self.host.change_early=True
        with self.assertRaisesRegex(m.Refusal,'transaction inputs'):self.run_enabled_fake()
        self.assertNotIn(('apply_provider_restart','provider'),self.host.calls);self.assertNotIn(('bootstrap_original','provider'),self.host.calls)
        self.assertEqual(set(self.host.holds),set(m.ROLES))
    def test_invalid_applied_receipt_does_not_advance_controller(self):
        self.host.bad_applied=True
        with self.assertRaisesRegex(m.Refusal,'applied provider'):self.run_enabled_fake()
        self.assertFalse(self.controller.provider_pinned);self.assertNotIn(('bootstrap_original','provider'),self.host.calls)
    def test_plist_completion_journal_failure_keeps_unresolved_intent(self):
        append=self.journal.append
        def fail(event_type,**data):
            if event_type=='mutation_completed' and data.get('operation')=='apply_provider_restart':raise OSError('simulated durable completion failure')
            return append(event_type,**data)
        with patch.object(self.journal,'append',side_effect=fail),self.assertRaises(OSError):self.run_enabled_fake()
        self.assertTrue(self.host.applied);self.assertFalse(self.controller.provider_pinned);self.assertNotIn(('bootstrap_original','provider'),self.host.calls)
        with patch.object(m,'PRODUCTION_CONTROLLER_ENABLED',True):view=m.inspect_recovery(self.journal,self.host)
        self.assertEqual(view['pending_mutation']['operation'],'apply_provider_restart');self.assertFalse(view['automatic_resume_allowed'])
    def test_bootstrap_failure_never_releases_holds_or_replays(self):
        self.host.fail=('bootstrap_original','provider')
        with self.assertRaises(RuntimeError):self.run_enabled_fake()
        self.assertTrue(self.controller.provider_pinned);self.assertEqual(set(self.host.holds),set(m.ROLES));prior=list(self.host.calls)
        with self.assertRaisesRegex(m.Refusal,'never replay'):self.run_enabled_fake()
        self.assertEqual(prior,self.host.calls)

if __name__=='__main__':unittest.main(verbosity=2)
