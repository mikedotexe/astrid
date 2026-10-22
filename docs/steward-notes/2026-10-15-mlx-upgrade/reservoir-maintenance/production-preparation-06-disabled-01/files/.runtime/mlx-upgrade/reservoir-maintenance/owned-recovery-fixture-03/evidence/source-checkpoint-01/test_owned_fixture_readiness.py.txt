"""Tiny fake-only readiness/order/deadline tests; no OS or child commands."""
from __future__ import annotations
import copy
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import owned_fixture_coordinator as c
import owned_fixture_readiness as ready
import owned_fixture_records as r
import owned_recovery_host as h
from owned_fixture_plan import proposed_schedule


class Readiness(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);self.root=Path(tmp.name).resolve()
        self.step=next(x for x in proposed_schedule()['executor_plan'] if ready.selected(x))
        self.now=10.;self.active=False;self.order=[];self.parent={'pid':100,'started_at':'inert','argv':['fake']}
        self.old={'pid':101,'started_at':'old','argv':['fake-worker']};self.holder={'pid':102,'started_at':'held','argv':['fake-holder']}
        self.guard={'inert':'exact-guard'};self.heads={'controller_head_sha256':'c','host_operations_sha256':'h','recovery_head_sha256':'r'}
        self.pending={'target':'inert','job_loaded':True,'process':self.old,'listener_pids':[],'held':None}
        self.held={'target':'inert','job_loaded':True,'process':self.holder,'listener_pids':[],'held':{'guard':self.guard}}
        self.inspection={'roles':{'provider':{'observed':self.held}},'inert':'full-report'}
        self.expected={**self.heads,'inspection_sha256':r.digest(self.inspection)}
        case=self
        class Session:
            def __init__(self,*args):self.begin={'inert':'session'}
            def __enter__(self):case.active=True;case.order.append('lock');return self
            def __exit__(self,*args):case.order.append('unlock');case.active=False
        def inspect(journal):
            self.assertTrue(self.active);self.order.append('inspection');return self.inspection
        self.host=SimpleNamespace(root=self.root,plan={'timeout_s':90},spec={'inert':'spec'},
            monotonic=lambda:self.now,sleep=self.sleep,executor_identity=lambda:self.parent,guard=lambda:self.guard,
            m=SimpleNamespace(Journal=lambda _:object()),io=SimpleNamespace(observations=[]),recovery_inspection=inspect)
        self.core={'transaction_execution':SimpleNamespace(ExecutionSession=Session),
            'recovery_ledger':SimpleNamespace(read_ledger=lambda *_:{}),
            'recovery_phases':SimpleNamespace(checkpoint_from_inspection=lambda *_:{**self.heads,'action_id':self.step['action_id']}),
            'production_recovery_driver':SimpleNamespace(expected_heads=lambda _:self.expected,validate_phase_observation=lambda *_,**kw:None)}
        self.operation=SimpleNamespace(core=self.core,release_ref={'path':'inert','sha256':'s'})
        self.claim=r.write(self.root/'claim.json',{'type':'claim','action':'executor:'+self.step['id']})
        self.enterContext(mock.patch.object(c,'owned_release',return_value={'artifacts':{'inert':'s'},'coordination':{'inert':'pause'}}))
        self.enterContext(mock.patch.object(ready,'basis',return_value=('provider',self.old,{'prior_debug':{},'prior_signal':{},'previous_exit':{}})))
        self.head=self.enterContext(mock.patch.object(ready,'stable_heads',return_value=self.heads))
        self.enterContext(mock.patch.object(ready,'observe_other',return_value={'inert':'other-exact'}))
        self.transition=self.enterContext(mock.patch.object(h,'readiness_transition',side_effect=self.transition_ready))
        self.prepare=self.enterContext(mock.patch.object(c,'prepare_executor',side_effect=self.prepare_request))

    def sleep(self,seconds):self.order.append(('sleep',seconds));self.now+=seconds
    def transition_ready(self,*args):
        self.assertTrue(self.active);self.order.append('sample');self.now+=.1
        return self.held,None
    def prepare_request(self,root,release,step,host,*,expected_heads,readiness):
        self.assertTrue(self.active);self.order.append('request');self.assertEqual(expected_heads,self.expected)
        self.assertEqual(r.check_reference(readiness)['final_inspection']['sha256'],r.reference(self.root/'readiness'/(self.step['id']+'-inspection.json'))['sha256'])
        return self.root/'executor-inert'

    def test_ready_is_observed_before_authoritative_heads_request_and_unlock(self):
        _,ref,deadline=ready.prepare(self.operation,self.host,self.step,self.claim)
        self.assertEqual(self.order,['lock','sample','inspection','request','unlock']);self.assertFalse(self.active)
        value=r.check_reference(ref);self.assertEqual(value['phase_deadline_monotonic'],310.);self.assertEqual(deadline,310.)
        self.assertEqual(value['readiness_deadline_monotonic'],100.);self.assertEqual(value['parent_claim'],self.claim)
        request={'step':self.step,'readiness':ref,'release':self.operation.release_ref,'root':str(self.root),
                 'coordinator_identity':self.parent,'expected_heads':self.expected}
        ready.validate_request(self.host,request)
        with self.assertRaisesRegex(ValueError,'attempted'):ready.prepare(self.operation,self.host,self.step,self.claim)

    def test_delayed_holder_wait_does_not_change_original_phase_budget(self):
        states=[(self.pending,self.old),(self.held,None)]
        def observe(*args):self.order.append('sample');self.now+=6.;return states.pop(0)
        self.transition.side_effect=observe
        _,ref,deadline=ready.prepare(self.operation,self.host,self.step,self.claim)
        value=r.check_reference(ref);self.assertEqual(len(value['samples']),2);self.assertEqual(deadline,310.)
        self.assertLess(value['samples'][-1]['completed_monotonic'],100.)
        self.assertEqual(self.order.count('inspection'),1);self.prepare.assert_called_once()

    def test_observation_completing_at_deadline_is_refused_without_request(self):
        def observe(*args):self.now=100.;return self.held,None
        self.transition.side_effect=observe
        with self.assertRaisesRegex(ValueError,'deadline'):ready.prepare(self.operation,self.host,self.step,self.claim)
        self.prepare.assert_not_called();self.assertNotIn('inspection',self.order)
        self.assertTrue((self.root/'readiness'/(self.step['id']+'-refused.json')).is_file())

    def test_deadline_checked_before_another_query_and_sleep_is_clamped(self):
        def observe(*args):self.now=99.98;return self.pending,self.old
        self.transition.side_effect=observe
        with self.assertRaisesRegex(ValueError,'deadline'):ready.prepare(self.operation,self.host,self.step,self.claim)
        self.transition.assert_called_once();self.assertAlmostEqual([x[1] for x in self.order if isinstance(x,tuple)][0],.02)
        self.prepare.assert_not_called()

    def test_authoritative_capture_overrun_refuses_without_request(self):
        def inspect(*args):self.now=100.;return self.inspection
        self.host.recovery_inspection=inspect
        with self.assertRaisesRegex(ValueError,'deadline'):ready.prepare(self.operation,self.host,self.step,self.claim)
        self.prepare.assert_not_called()

    def test_changed_retained_head_refuses_before_any_query(self):
        self.head.side_effect=[self.heads,{**self.heads,'host_operations_sha256':'foreign'}]
        with self.assertRaisesRegex(ValueError,'history changed'):ready.prepare(self.operation,self.host,self.step,self.claim)
        self.transition.assert_not_called();self.prepare.assert_not_called()

    def test_pending_holder_pid_reuse_refuses_before_authoritative_capture(self):
        first={**self.pending,'process':self.holder,'holder_pending':True}
        second={**self.held,'process':{**self.holder,'started_at':'different'}}
        self.transition.side_effect=[(first,None),(second,None)]
        with self.assertRaisesRegex(ValueError,'identity changed'):ready.prepare(self.operation,self.host,self.step,self.claim)
        self.prepare.assert_not_called();self.assertNotIn('inspection',self.order)

    def test_old_worker_or_listener_never_accepted_as_ready(self):
        for old_seen,listeners in ((self.old,[]),(None,[101])):
            self.transition.side_effect=None;self.transition.return_value=({**self.held,'listener_pids':listeners},old_seen)
            with self.subTest(old=old_seen),self.assertRaisesRegex(ValueError,'retains'):
                ready.prepare(self.operation,self.host,self.step,self.claim)
            refused=self.root/'readiness'/(self.step['id']+'-refused.json');refused.unlink()
        self.prepare.assert_not_called()

    def test_only_two_fixed_main_steps_receive_readiness(self):
        selected=[x for x in proposed_schedule()['executor_plan'] if ready.selected(x)]
        self.assertEqual([x['action_id'] for x in selected],['held_verified:provider','held_verified:reservoir'])
        self.assertTrue(all(x['mode']=='reconcile' and x['kind']=='resume_one' for x in selected))
        with self.assertRaisesRegex(ValueError,'unapproved'):ready.prepare(self.operation,self.host,{**self.step,'mode':'execute'},self.claim)

    def test_child_rejects_changed_readiness_without_refresh(self):
        _,ref,_=ready.prepare(self.operation,self.host,self.step,self.claim)
        request={'step':self.step,'readiness':ref,'release':self.operation.release_ref,'root':str(self.root),
                 'coordinator_identity':self.parent,'expected_heads':{**self.expected,'inspection_sha256':'foreign'}}
        with self.assertRaisesRegex(ValueError,'binding differs'):ready.validate_request(self.host,request)
        self.assertEqual(self.order.count('sample'),1)


class Transition(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);self.root=Path(tmp.name).resolve()
        self.old={'pid':101,'started_at':'old','argv':['worker']};self.holder={'pid':102,'started_at':'held','argv':['holder']}
        self.guard={'transaction_id':'inert'};self.processes={101:self.old};self.job=(True,101)
        self.io=SimpleNamespace(job=lambda role:self.job,process=lambda pid:self.processes.get(pid),
            port_pids=lambda port:[],listener_ports=lambda pid:[])
        self.host=SimpleNamespace(root=self.root,plan={'roles':{'provider':{'identity':self.old}}},
            core={'transaction_execution':SimpleNamespace(require_active=mock.Mock())},signalled={('provider',101,'old')},
            guard=lambda:self.guard,env=SimpleNamespace(assert_hold=mock.Mock(),command_matches=lambda a,b:a==b),
            control=lambda role:self.root,io=self.io,remember_ports=lambda role:None,
            known_ports={'provider':{45000}},ports={'provider':45000},target=lambda role:'inert-target',
            hold_argv=lambda *args:['holder'])
    def observe(self):return h.readiness_transition(self.host,'provider',self.old,self.guard)

    def test_draining_old_then_loaded_gap_are_pending_not_held(self):
        view,old=self.observe();self.assertIsNone(view['held']);self.assertEqual(old,self.old)
        self.processes={}
        view,old=self.observe();self.assertIsNone(view['held']);self.assertIsNone(old)
        self.job=(True,None);view,_=self.observe();self.assertIsNone(view['held'])

    def test_exact_holder_without_receipt_is_pending(self):
        self.job=(True,102);self.processes={102:self.holder}
        view,old=self.observe();self.assertTrue(view['holder_pending']);self.assertIsNone(view['held']);self.assertIsNone(old)

    def test_foreign_holder_reused_old_pid_or_listener_refuses(self):
        for processes,job,ports in (({102:{**self.holder,'argv':['foreign']}},(True,102),[]),
                                  ({101:{**self.old,'started_at':'reused'}},(True,101),[]),
                                  ({102:self.holder},(True,102),[999]),({},(False,None),[])):
            self.processes=processes;self.job=job;self.io.port_pids=lambda port:ports
            with self.subTest(job=job,processes=processes),self.assertRaises(ValueError):self.observe()

    def test_receipt_publication_uses_existing_strict_holder_predicate(self):
        self.job=(True,102);self.processes={102:self.holder};(self.root/'held').mkdir(mode=0o700)
        r.write(self.root/'held/102.json',{'inert':'receipt'})
        view={'process':self.holder,'held':{'guard':self.guard}}
        self.host.inspect_role=mock.Mock(return_value=view)
        self.assertEqual(self.observe(),(view,None));self.host.inspect_role.assert_called_once_with('provider')



class SharedPhaseBudget(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);self.root=Path(tmp.name).resolve()
        executors=self.root/'executors';executors.mkdir(mode=0o700)
        step=next(x for x in proposed_schedule()['executor_plan'] if ready.selected(x));self.directory=executors/step['id'];self.directory.mkdir(mode=0o700)
        self.now=10.;self.deadline=30.;self.parent={'pid':100,'started_at':'inert-parent','argv':['inert-parent']}
        self.host=SimpleNamespace(root=self.root,executor_identity=lambda:self.parent,check_owned=mock.Mock(),
            env=SimpleNamespace(command_matches=lambda actual,expected:actual==expected))
        readiness=r.write(self.root/'readiness.json',{'phase_deadline_monotonic':self.deadline})
        self.release={'python':'/inert/python'};self.release_ref=r.write(self.root/'release.json',self.release)
        self.request={'step':step,'release':self.release_ref,'root':str(self.root),'readiness':readiness}
        self.request_ref=r.write(self.directory/'request.json',self.request)
        self.child=mock.MagicMock(pid=222,returncode=None)
        def wait(**kwargs):self.child.returncode=0;self.now+=1.;return 0
        self.child.wait.side_effect=wait
        def spawn(argv,**kwargs):
            r.write(self.directory/'self-start.json',{'identity':{'pid':222,'started_at':'inert-child','argv':argv},'request':self.request_ref})
            return self.child
        self.spawn=mock.Mock(side_effect=spawn)
        self.enterContext(mock.patch.object(c,'FIXTURE_EXECUTION_ENABLED',True))
        self.enterContext(mock.patch.object(c,'owned_release',return_value=self.release))
        self.enterContext(mock.patch.object(c.time,'monotonic',side_effect=lambda:self.now))

    def launch(self):return c.launch_executor(self.directory,self.host,spawn=self.spawn,phase_deadline=self.deadline)

    def test_delayed_created_publication_reduces_actual_wait_budget(self):
        original=c.write
        def write(path,value):
            result=original(path,value)
            if Path(path).name=='child-created.json':self.now+=11.
            return result
        with mock.patch.object(c,'write',side_effect=write):out=self.launch()
        self.child.wait.assert_called_once_with(timeout=9)
        self.assertEqual(out['wait_started_monotonic'],21.);self.assertEqual(out['completed_monotonic'],22.)
        self.assertEqual(r.read(self.directory/'launch.json')['timeout_s'],20)
        self.assertEqual(r.read(self.directory/'child-created.json')['budget_timeout_s'],20)

    def test_created_publication_exhausting_budget_retains_child_without_wait_or_force(self):
        original=c.write
        def write(path,value):
            result=original(path,value)
            if Path(path).name=='child-created.json':self.now=30.
            return result
        with mock.patch.object(c,'write',side_effect=write),self.assertRaisesRegex(ValueError,'exhausted'):self.launch()
        self.child.wait.assert_not_called();self.child.kill.assert_not_called();self.child.terminate.assert_not_called()
        self.assertEqual(r.read(self.directory/'unresolved.json')['pid'],222)

    def test_natural_return_at_deadline_is_recorded_reaped_but_never_accepted(self):
        def wait(**kwargs):self.now=30.;self.child.returncode=0;return 0
        self.child.wait.side_effect=wait
        with self.assertRaisesRegex(ValueError,'after original phase'):self.launch()
        row=r.read(self.directory/'unresolved.json');self.assertTrue(row['naturally_reaped']);self.assertEqual(row['known_returncode'],0)
        self.assertFalse((self.directory/'exit.json').exists());self.child.kill.assert_not_called()
        with self.assertRaisesRegex(ValueError,'never relaunch'):self.launch()
        self.spawn.assert_called_once()

    def test_final_acceptance_after_slow_validation_cannot_extend_deadline(self):
        self.host.check_owned.side_effect=lambda:setattr(self,'now',30.)
        with self.assertRaisesRegex(ValueError,'acceptance exceeded'):self.launch()
        self.assertTrue(r.read(self.directory/'unresolved.json')['naturally_reaped'])
        self.child.kill.assert_not_called();self.child.terminate.assert_not_called()

    def test_expired_before_popen_never_starts_child(self):
        self.now=30.
        with self.assertRaisesRegex(ValueError,'exhausted'):self.launch()
        self.spawn.assert_not_called()



if __name__=='__main__':unittest.main()
