"""Isolated retained recovery contracts, with inert IO and synthetic records."""
import copy
import json
import os
from pathlib import Path
import tempfile
import time
import types
import unittest
from unittest.mock import Mock, patch

import production_host as h
import production_retained as r
import production_recovery_inspect as inspector
import recovery_ledger
import transaction_execution


class RetainedTests(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);self.root=Path(tmp.name).resolve()
        self.old={'pid':111,'started_at':'Wed Oct 14 12:00:00 2026','argv':['python','service.py']}
        self.holder={'pid':222,'started_at':'Thu Oct 15 12:00:00 2026','argv':['python','holder.py']}
        self.plan={'transaction_id':'a'*32,'actor':'test','host_binding':{'uid':os.getuid()},
                   'roles':{role:{'identity':copy.deepcopy(self.old)} for role in h.m.ROLES},
                   'input_hashes':{'reservoir_candidate':'a'*64,'interpreter':'b'*64,'reservoir_settings':'c'*64,
                                   'provider_restart_settings':'d'*64}}
        self.guard={'schema':'model_environment_hold_v1','transaction_id':'a'*32,'actor':'test'}
        self.spec={'plan':self.plan,'transaction_root':str(self.root),'provider_restart_argv':['python','service.py','--model','snapshot']}
        self.checkpoint={'schema':'reservoir_recovery_checkpoint_v1','transaction_id':'a'*32,
            'plan_sha256':h.m.digest(self.plan),'preparation_sha256':h.p.preparation_digest(self.spec),
            'recovery_head_sha256':'e'*64,'controller_head_sha256':'f'*64,'host_counter':0,
            'guards':{role:self.guard for role in h.m.ROLES},'anchors':{'fixed':'saved'},'receipts':{},
            'signal_times':{'provider':123456},'armed_attempts':['provider'],'armed_identities':{},
            'signal_attempts':['provider'],'signalled_identities':{},'startups':{},'replacements':{},
            'repaired':False,'provider_pinned':False}
        self.inspection={'evidence_valid':True,'errors':[],'artifacts':{},'observed_after':{'snapshot':'same'},
            'installed_variants':{},'host_operations':{'events':[]}}
        self.session=types.SimpleNamespace(begin={'session':'synthetic'})
        self.host=h.ProductionHost.__new__(h.ProductionHost)
        self.host.spec=self.spec;self.host.plan=self.plan;self.host.root=self.root;self.host.journal_path=self.root/'controller'
        self.host.io=Mock();self.host.io.process.return_value=None;self.host.production_admission=Mock(return_value={})
        self.host.receipts={};self.host.guards={role:self.guard for role in h.m.ROLES};self.host.startups={}
        self.host.env=Mock();self.host.control=lambda role:self.root/('hold-'+role)
        self.host.inspect_role=Mock(return_value={'process':self.holder,'listener_pids':[],
            'held':{'identity':self.holder,'guard':self.guard}})
        self.host._absent=Mock();self.host.anchors={'fixed':'saved'}
        self.host._recovery_validated={'checkpoint':self.checkpoint,'inspection':self.inspection,'ledger_head':'e'*64}
        self.host.recovery_inspection=Mock(return_value=self.inspection)
        self.history={'events':[],'pairs':{}}
        self.ledger={'header':{},'head_sha256':'e'*64,'records':[]}
        self.inspection['recovery_ledger']=copy.deepcopy(self.ledger)
        self.patchers=[]
        def stub(target,name,**kwargs):
            item=patch.object(target,name,**kwargs);self.patchers.append(item);return item.start()
        self.gate=stub(h,'PRODUCTION_EXECUTION_ENABLED',new=True)
        stub(transaction_execution,'require_active',return_value=self.session)
        self.read=stub(recovery_ledger,'read_ledger',side_effect=lambda *a:self.ledger)
        stub(inspector,'validate_retained',side_effect=lambda *a:(self.plan,{},self.inspection['installed_variants']))
        stub(inspector,'snapshot',side_effect=lambda *a:self.inspection['observed_after'])
        stub(inspector,'anchors',return_value=self.checkpoint['anchors'])
        stub(inspector,'controller_records',return_value={'last_sha256':'f'*64})
        stub(inspector,'host_records',side_effect=lambda *a:self.history)
        self.kill=stub(h.os,'kill',side_effect=AssertionError('no signal'))
        self.spawn=stub(h.subprocess,'run',side_effect=AssertionError('no child/command'))
        self.addCleanup(lambda:[item.stop() for item in reversed(self.patchers)])

    def claim(self,action):
        self.checkpoint['action_id']=action
        self.host._recovery_validated={'checkpoint':copy.deepcopy(self.checkpoint),'inspection':copy.deepcopy(self.inspection),'ledger_head':'e'*64}
        operation,_,kind=action.partition(':')
        self.ledger['records']=[{'type':'reconciliation_claim','action_id':action,
            'checkpoint_sha256':h.m.digest(self.checkpoint),'inspection_sha256':h.m.digest(self.inspection),
            'executor_session':self.session.begin,'context':r.context_for(self.host,kind if operation=='witness' else None)}]

    def reconcile(self,action):
        self.claim(action);return r.reconcile(self.host,action,self.checkpoint,self.inspection)

    def test_default_gate_blocks_before_ledger_or_live_io(self):
        with patch.object(h,'PRODUCTION_EXECUTION_ENABLED',False),self.assertRaisesRegex(h.m.Refusal,'disabled'):
            r.reconcile(self.host,'signal_stop:provider',self.checkpoint,self.inspection)
        self.read.assert_not_called();self.host.io.process.assert_not_called();self.kill.assert_not_called()

    def test_exact_holder_resolves_effect_without_claiming_signal_or_rearming(self):
        for operation,key in [('arm_next_spawn','debug_intent'),('signal_stop','signal_intent')]:
            self.history['pairs']={(key,'provider'):{'intent':{'identity':self.old},'result':None}}
            proof=self.reconcile(operation+':provider')
            self.assertEqual(proof['result']['effect'],'owned_holder_observed')
            self.assertNotIn('armed',proof['result']);self.assertNotIn('signal',proof['result'])
            self.assertFalse(proof['evidence']['historical_signal_or_debug_return_inferred'])
        self.kill.assert_not_called();self.spawn.assert_not_called()

    def test_surviving_original_or_missing_intent_cannot_adopt_holder(self):
        with self.assertRaisesRegex(h.m.Refusal,'intent'):self.reconcile('signal_stop:provider')
        self.history['pairs']={('signal_intent','provider'):{'intent':{'identity':self.old},'result':None}}
        self.host.io.process.return_value=self.old
        with self.assertRaisesRegex(h.m.Refusal,'original exit'):self.reconcile('signal_stop:provider')

    def test_removed_holder_needs_exact_intent_and_no_survivor(self):
        self.history['pairs']={('held_bootout_intent','provider'):{'intent':{'identity':self.holder},'result':None}}
        proof=self.reconcile('remove_held:provider')
        self.assertEqual(proof['result'],{'removed_identity':self.holder})
        self.host.io.process.return_value=self.holder
        with self.assertRaisesRegex(h.m.Refusal,'survives'):self.reconcile('remove_held:provider')
        self.host._absent.assert_called_with('provider');self.spawn.assert_not_called()

    def test_claim_must_bind_validated_checkpoint_and_current_session(self):
        self.claim('held_verified:provider');self.ledger['records'][0]['executor_session']={'foreign':True}
        with self.assertRaisesRegex(h.m.Refusal,'current claim'):
            r.reconcile(self.host,'held_verified:provider',self.checkpoint,self.inspection)
        self.host.inspect_role.assert_not_called()

    def test_revalidate_recomputes_and_reserves_unknown_signal_attempt(self):
        import production_recovery
        with patch.object(production_recovery,'checkpoint_from_inspection',return_value=self.checkpoint):
            r.revalidate(self.host,self.checkpoint,self.inspection)
        self.assertIn('provider',self.host.armed)
        self.assertNotIn(('provider',111,self.old['started_at']),self.host.armed)
        self.assertIn(('provider',111,self.old['started_at']),self.host.signalled)
        self.assertEqual(self.host.signal_times,{'provider':123456})
        self.assertEqual(self.host.anchors,{'fixed':'saved'})

    def test_arbitrary_checkpoint_cannot_hydrate(self):
        import production_recovery
        altered=dict(self.checkpoint,repaired=True)
        with patch.object(production_recovery,'checkpoint_from_inspection',return_value=self.checkpoint),self.assertRaisesRegex(h.m.Refusal,'authoritative'):
            r.revalidate(self.host,altered,self.inspection)
        self.host.production_admission.assert_not_called()

    def test_every_accepted_witness_and_replacement_is_revalidated(self):
        import production_recovery
        self.checkpoint['receipts']={'provider_drain':{'passed':True}}
        self.checkpoint['replacements']={'provider':self.holder}
        with patch.object(production_recovery,'checkpoint_from_inspection',return_value=self.checkpoint),\
             patch.object(r,'validate_existing_witness',return_value={'passed':True}) as witness,\
             patch.object(r,'ready_identity',side_effect=h.m.Refusal('unhealthy')) as ready,\
             self.assertRaisesRegex(h.m.Refusal,'unhealthy'):
            r.revalidate(self.host,self.checkpoint,self.inspection)
        witness.assert_called_once();ready.assert_called_once()
        self.host.production_admission.assert_not_called()

    def test_bootstrap_birth_boundary_and_exact_listener_are_required(self):
        with self.assertRaisesRegex(h.m.Refusal,'birth boundary'):
            r.bootstrap_effect(self.host,'provider',{'intent':{}})
        ident={'pid':333,'started_at':time.strftime('%a %b %d %H:%M:%S %Y',time.localtime()),
               'argv':self.spec['provider_restart_argv']}
        self.host.io.role.return_value={'process':ident,'listener_pids':[333]}
        self.host.inspect_role.return_value={'process':ident,'listener_pids':[333]}
        self.host.receipts={'startup':{'passed':True}}
        self.host.io.health.return_value={'ready':True,'worker':{'phase':'generating','last_generation_error':None},
                                         'reservoir':{'status':'connected'}}
        result,proof=r.bootstrap_effect(self.host,'provider',{'intent':{'wall_time_ns':time.time_ns()}})
        self.assertEqual(result,{'identity':ident});self.assertFalse(proof['historical_command_return_inferred'])
        self.host.io.health.return_value['worker']['last_generation_error']='failure'
        with self.assertRaisesRegex(h.m.Refusal,'readiness'):
            r.bootstrap_effect(self.host,'provider',{'intent':{'wall_time_ns':time.time_ns()}})

    def test_provider_ready_adoption_does_not_compare_current_journal(self):
        identity=self.holder;self.host.startups={'provider':{'identity':identity}}
        saved=self.root/'saved-journal.json';saved.write_text('{}')
        reference={'path':str(saved),'sha256':h.p.sha(saved)}
        self.host.receipts={'provider_drain':{'evidence':{'journal':reference}},'startup':{'passed':True}}
        context=r.context_for(self.host,'provider_ready')
        receipt=r.state.envelope(context,'provider_ready',{'identity':identity,'health':{'ready':True,
            'worker':{'phase':'ready','last_generation_error':None},'reservoir':{'status':'connected'}},
            'journal_before_bootstrap':reference})
        (self.root/'provider_ready-witness.json').write_text(json.dumps(receipt))
        with patch.object(r,'ready_identity') as ready,patch.object(r.p,'JOURNAL',self.root/'nonexistent-current-journal'):
            self.assertEqual(r.validate_existing_witness(self.host,'provider_ready',context,adoption=True),receipt)
        ready.assert_called_once()

    def test_span_validation_preserves_original_offset_and_detects_prefix_rewrite(self):
        live=self.root/'live.log';live.write_bytes(b'prefix\nold-drain\nnew-work\n')
        info=live.stat();prefix=b'prefix\n';tail=b'old-drain\n';saved=self.root/'drain.log';saved.write_bytes(tail)
        anchor={'path':str(live),'device':info.st_dev,'inode':info.st_ino,'start_offset':len(prefix),
                'prefix_sha256':__import__('hashlib').sha256(prefix).hexdigest()}
        span={k:anchor[k] for k in ('device','inode','start_offset')}
        span.update(end_offset=len(prefix)+len(tail),sha256=h.p.sha(saved))
        r.validate_span(anchor,span,saved)
        live.write_bytes(b'broken\nold-drain\nnew-work\n')
        with self.assertRaisesRegex(h.m.Refusal,'prefix'):r.validate_span(anchor,span,saved)

class IntegratedDriverTests(unittest.TestCase):
    def test_two_steps_share_real_ledger_host_and_inspector_without_original_journal_rewrite(self):
        # Reuse synthetic fixed-path preparation; no actual command/service IO.
        from test_production_recovery_inspect import RecoveryTests
        from production_recovery_driver import resume_one, expected_heads
        from production_recovery import checkpoint_from_inspection
        fixture=RecoveryTests('test_empty_transaction_and_execution_review_exclusion')
        fixture.setUp();self.addCleanup(fixture.doCleanups)
        io=fixture.io
        executor={'pid':os.getpid(),'started_at':'synthetic-test-executor','argv':['fake-python','test-only']}
        io.processes[os.getpid()]=executor
        def create_guard(control,transaction,actor):
            control.mkdir(mode=0o700)
            guard={'schema':'model_environment_hold_v1','transaction_id':transaction,'actor':actor}
            h.m.write_new(control/'hold.json',guard);return guard
        env=types.SimpleNamespace(command_matches=lambda actual,expected:actual==expected,create_hold=create_guard,
            assert_hold=lambda control,guard: self.assertEqual(h.m.read_record(control/'hold.json'),guard))
        with patch.object(h.p,'load_hold',return_value=env),patch.object(h,'PRODUCTION_EXECUTION_ENABLED',True),\
             patch.object(h.m,'PRODUCTION_CONTROLLER_ENABLED',True),\
             patch.object(h.m,'validate_production_admission',return_value={}),\
             patch.object(h.m,'preflight',return_value={'passed':True,'production_admission':{}}):
            host=h.ProductionHost.from_verified_recovery(fixture.prep,fixture.journal,io=io)
            host.production_admission=Mock(return_value={})
            for expected_action in ('preflight','create_hold:provider'):
                journal=h.m.Journal(fixture.journal)
                report=host.recovery_inspection(journal)
                self.assertTrue(report['evidence_valid'],report['errors'])
                checkpoint=checkpoint_from_inspection(host.spec,report,recovery_ledger.read_ledger(host.root,host.plan))
                self.assertEqual(checkpoint['action_id'],expected_action)
                result=resume_one(journal,host,expected=expected_heads(checkpoint))
                self.assertEqual(result['action_id'],expected_action)
                self.assertFalse(result['automatic_next_action'])
            self.assertEqual(h.m.Journal(fixture.journal).events,[])
            report=host.recovery_inspection(h.m.Journal(fixture.journal))
            self.assertTrue(report['evidence_valid'],report['errors'])
            checkpoint=checkpoint_from_inspection(host.spec,report,recovery_ledger.read_ledger(host.root,host.plan))
            self.assertEqual(checkpoint['action_id'],'arm_next_spawn:provider')
            self.assertEqual(checkpoint['host_counter'],2)
            self.assertEqual(host.counter,2)
            self.assertEqual(report['roles']['provider']['guard'],fixture.guard())
            self.assertEqual(len(report['recovery_ledger']['records']),4)
            # A real retained Host result exists but its driver return was lost.
            # Record only synthetic debug evidence; never execute sudo/debug.
            def fake_arm(role,guard,old):
                argv=['/usr/bin/sudo','--','/bin/launchctl','debug',host.target(role),'--program',
                    inspector.HOLDER_PYTHON,'--',str(host.root/'model_environment.py'),'launch','--control-root',
                    str(host.control(role)),'--hold-only',host.plan['transaction_id']]
                host.event('debug_intent',role=role,command=argv,identity=old)
                host.event('debug_result',role=role,returncode=0)
                raise RuntimeError('synthetic interruption after recorded result')
            with patch.object(host,'arm_next_spawn',side_effect=fake_arm) as arm:
                with self.assertRaisesRegex(RuntimeError,'synthetic interruption'):
                    resume_one(h.m.Journal(fixture.journal),host,expected=expected_heads(checkpoint))
                report=host.recovery_inspection(h.m.Journal(fixture.journal))
                self.assertTrue(report['evidence_valid'],report['errors'])
                checkpoint=checkpoint_from_inspection(host.spec,report,recovery_ledger.read_ledger(host.root,host.plan))
                self.assertTrue(checkpoint['requires_reconciliation'])
                recovered=resume_one(h.m.Journal(fixture.journal),host,expected=expected_heads(checkpoint),mode='reconcile')
                self.assertEqual(recovered['result'],{'armed':True})
                arm.assert_called_once()
            report=host.recovery_inspection(h.m.Journal(fixture.journal))
            self.assertTrue(report['evidence_valid'],report['errors'])
            checkpoint=checkpoint_from_inspection(host.spec,report,recovery_ledger.read_ledger(host.root,host.plan))
            self.assertEqual(checkpoint['action_id'],'signal_stop:provider')
            self.assertFalse(checkpoint['requires_reconciliation'])
            self.assertEqual(checkpoint['armed_identities']['provider'],fixture.plan['roles']['provider']['identity'])
            self.assertEqual(h.m.Journal(fixture.journal).events,[])
        fixture.commands.assert_not_called()


if __name__=='__main__':unittest.main(verbosity=2)
