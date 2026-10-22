"""Finite abort ordering and refusal tests. All service observations/effects are fake."""
from __future__ import annotations
import copy
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock,patch

import abort_driver as d
import abort_evidence as e


def target():
    return {'workers':{'provider':{'pid':9267,'started_at':'old','argv':['python','provider']},
                       'reservoir':{'pid':9217,'started_at':'old','argv':['python','reservoir']}},
            'provider_holder':{'pid':10721,'started_at':'held','argv':['python','hold-provider']},
            'ports':{'provider':50551,'reservoir':50552},'release':{'path':'old','sha256':'old'},
            'snapshot':{'path':'snapshot','sha256':'snapshot'}}


class FakeHost:
    scope='failed_owned_fixture_abort_only'
    def __init__(self):
        self.t=target();self.calls=[];self.clock=0.;self.signalled=False;self.settled=False
        self.removed=set();self.released=set();self.waits=0;self.fail=None;self.foreign=False
        self.holder={'pid':11001,'started_at':'new','argv':['python','hold-reservoir']}
    def verify(self,*,saved=False,provider_released=False):
        self.calls.append(('verify',saved,provider_released))
        if saved:
            e.require(self.settled,'saved proof requested before holder')
            e.require(self.fail!='saved','wrong saved state')
        return {'saved':saved,'state':e.STATE}
    def inspect(self,role):
        if role in self.removed:return {'loaded':False,'identity':None,'held':False,'port_pids':[]}
        if role=='provider':
            return {'loaded':True,'identity':self.t['provider_holder'],'held':True,'port_pids':[]}
        if not self.signalled:
            return {'loaded':True,'identity':self.t['workers']['reservoir'],'held':False,'port_pids':[9217]}
        if self.foreign:return {'loaded':True,'identity':{'pid':2,'started_at':'foreign','argv':['foreign']},'held':False,'port_pids':[]}
        self.waits+=1
        if self.fail=='timeout' or self.waits==1:
            return {'loaded':True,'identity':self.t['workers']['reservoir'],'held':False,'port_pids':[]}
        if self.waits==2:return {'loaded':True,'identity':None,'held':False,'port_pids':[]}
        if self.waits==3:return {'loaded':True,'identity':self.holder,'held':False,'holder_pending':True,'port_pids':[]}
        self.settled=True
        return {'loaded':True,'identity':self.holder,'held':True,'port_pids':[]}
    def create_guard(self,record):self.calls.append(('guard',));return {'guard':'new'}
    def debug(self,record):
        self.calls.append(('debug',))
        if self.fail=='debug':raise RuntimeError('uncertain debug')
        return {'returncode':1 if self.fail=='nonzero' else 0,'naturally_reaped':True}
    def signal_reservoir(self,record):
        self.calls.append(('signal',));self.signalled=True
        if self.fail=='signal':raise RuntimeError('unknown signal outcome')
        return {'signal':'SIGTERM'}
    def process(self,pid):
        if pid==9217 and not self.settled:return self.t['workers']['reservoir']
        return None
    def verify_signal_boundary(self,intent):
        self.calls.append(('signal_boundary',intent['action']))
        e.require(self.fail!='boundary','save predates signal')
    def capture_saved(self):
        e.require(self.settled and not self.removed,'capture must precede held removal')
        self.calls.append(('capture_saved',));return {'state.json':{'utf8':'11/7'},'reservoir.events.jsonl':{'utf8':'final'}}
    def remove_held(self,role,expected,record):
        e.require(self.settled,'stateful bootout');self.calls.append(('remove',role));self.removed.add(role)
        if self.fail=='bootout':raise RuntimeError('unknown bootout')
        return {'returncode':0,'naturally_reaped':True}
    def release_guard(self,role,expected,record):
        e.require(role in self.removed,'guard released while loaded');self.calls.append(('release',role));self.released.add(role)
        return {'released':True}
    def job(self,role):return (False,None) if role in self.removed else (True,self.holder['pid'])
    def port_pids(self,role):return []
    def final_evidence(self):return {'state':'11/7','old_workers_absent':True}
    def monotonic(self):return self.clock
    def sleep(self,seconds):self.clock+=seconds


class DriverTests(unittest.TestCase):
    def setUp(self):
        self.host=FakeHost();self.rows=[]
    def record(self,kind,**values):
        row={'type':kind,'sequence':len(self.rows)+1,'monotonic':0.,'wall_time_ns':1,**values}
        self.rows.append(row);return row
    def run_sequence(self):return d.sequence(self.host,self.host.t,self.record)
    def test_success_saves_once_then_removes_only_holders(self):
        result=self.run_sequence()
        effects=[row for row in self.host.calls if row[0] in ('guard','debug','signal','remove','release')]
        self.assertEqual(effects,[('guard',),('debug',),('signal',),('remove','provider'),('release','provider'),
                                 ('remove','reservoir'),('release','reservoir')])
        self.assertEqual(result['state'],e.STATE);self.assertFalse(result['original_fixture_passed'])
        self.assertFalse(result['provider_replayed']);self.assertTrue(result['abort_executor_absence_requires_independent_closeout'])
        self.assertEqual(len([r for r in self.rows if r['type']=='intent']),7)
        save=next(r for r in self.rows if r['type']=='durable_save_verified')
        self.assertEqual(set(save['saved_artifacts']),{'state.json','reservoir.events.jsonl'})
    def test_settles_old_worker_no_pid_and_unpublished_receipt(self):
        self.run_sequence();self.assertGreaterEqual(self.host.waits,4);self.assertEqual(self.host.clock,.75)
    def test_unknown_debug_stops_without_signal(self):
        self.host.fail='debug'
        with self.assertRaisesRegex(RuntimeError,'uncertain'):self.run_sequence()
        self.assertNotIn(('signal',),self.host.calls)
    def test_nonzero_debug_stops_without_signal(self):
        self.host.fail='nonzero'
        with self.assertRaisesRegex(ValueError,'natural return'):self.run_sequence()
        self.assertNotIn(('signal',),self.host.calls)
    def test_unknown_signal_is_not_repeated(self):
        self.host.fail='signal'
        with self.assertRaisesRegex(RuntimeError,'unknown signal'):self.run_sequence()
        self.assertEqual(self.host.calls.count(('signal',)),1);self.assertFalse(self.host.removed)
    def test_foreign_replacement_stops_without_bootout(self):
        self.host.foreign=True
        with self.assertRaisesRegex(ValueError,'foreign replacement'):self.run_sequence()
        self.assertFalse(self.host.removed)
    def test_settle_timeout_retains_guards_and_workers(self):
        self.host.fail='timeout'
        with self.assertRaisesRegex(ValueError,'timed out'):self.run_sequence()
        self.assertEqual(self.host.clock,90.);self.assertFalse(self.host.removed);self.assertFalse(self.host.released)
    def test_truthy_observation_after_deadline_is_refused(self):
        def late():self.host.clock=2.;return True
        with self.assertRaisesRegex(ValueError,'timed out'):d.wait_for(self.host,late,'late',1)
    def test_observation_is_not_started_at_deadline(self):
        self.host.monotonic=Mock(side_effect=[0.,1.]);predicate=Mock()
        with self.assertRaisesRegex(ValueError,'timed out'):d.wait_for(self.host,predicate,'expired',1)
        predicate.assert_not_called()
    def test_sleep_is_clamped_to_original_remaining_budget(self):
        sleeps=[]
        def pending():self.host.clock=.9;return False
        def sleep(seconds):sleeps.append(seconds);self.host.clock+=seconds
        self.host.sleep=sleep
        with self.assertRaisesRegex(ValueError,'timed out'):d.wait_for(self.host,pending,'bounded',1)
        self.assertEqual(len(sleeps),1);self.assertAlmostEqual(sleeps[0],.1)
    def test_failed_final_state_never_boots_out(self):
        self.host.fail='saved'
        with self.assertRaisesRegex(ValueError,'wrong saved'):self.run_sequence()
        self.assertFalse(self.host.removed)
    def test_save_before_signal_never_boots_out(self):
        self.host.fail='boundary'
        with self.assertRaisesRegex(ValueError,'predates'):self.run_sequence()
        self.assertFalse(self.host.removed)
    def test_unknown_bootout_never_releases_guard_or_retries(self):
        self.host.fail='bootout'
        with self.assertRaisesRegex(RuntimeError,'unknown bootout'):self.run_sequence()
        self.assertEqual([r for r in self.host.calls if r[0]=='remove'],[('remove','provider')]);self.assertFalse(self.host.released)
    def test_original_reservoir_listener_is_required(self):
        original=self.host.inspect
        def bad(role):
            view=original(role)
            if role=='reservoir':view['port_pids']=[]
            return view
        self.host.inspect=bad
        with self.assertRaisesRegex(ValueError,'not ready'):self.run_sequence()
        self.assertNotIn(('guard',),self.host.calls)
    def test_existing_abort_root_refused_before_any_probes(self):
        with tempfile.TemporaryDirectory() as tmp,patch.object(e,'ABORT',Path(tmp).resolve()):
            with self.assertRaisesRegex(ValueError,'one-use'):d.preflight(self.host,self.host.t,{})
        self.assertEqual(self.host.calls,[])
    def test_ledger_reserves_action_before_effect_and_never_reuses(self):
        writes=[]
        with patch.object(d,'write_new',side_effect=lambda path,row:writes.append(copy.deepcopy(row))):
            ledger=d.Ledger({'executor_identity':{'pid':123}})
            ledger.record('intent',action='signal_reservoir')
            with self.assertRaisesRegex(ValueError,'already attempted'):ledger.record('intent',action='signal_reservoir')
        self.assertEqual(len(writes),1);self.assertEqual(writes[0]['action'],'signal_reservoir')


if __name__=='__main__':unittest.main()
