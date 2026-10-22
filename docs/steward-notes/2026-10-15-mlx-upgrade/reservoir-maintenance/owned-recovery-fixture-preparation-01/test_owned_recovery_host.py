"""Stdlib/fake IO integration; never launches a process, listener, or signal."""
from __future__ import annotations
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import shutil
import sys
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import uuid

import owned_fixture_records as r
import owned_recovery_host as h
from owned_recovery_core import CORE_ROOT, MODULES, load_core


class FakeIO:
    def __init__(self,root,token,python,binding):
        self.root,self.token,self.python,self.binding=root,token,python,binding
        self.now=1.;self.next_pid=800000;self.jobs={};self.processes={};self.ports={};self.fixture_host=None
        self.calls=[];self.state={'fixture_astrid':{'ticks':0},'fixture_minime':{'ticks':7}}
        self.executor={'pid':os.getpid(),'started_at':time.ctime(),'argv':[str(python),'-B','owned-fake-test']}
        self.debug_code=0
    def host_binding(self):return copy.deepcopy(self.binding)
    def monotonic(self):return self.now
    def sleep(self,seconds):self.now+=seconds
    def process(self,pid):return copy.deepcopy(self.executor if pid==os.getpid() else self.processes.get(pid))
    def job(self,role):return (role in self.jobs,self.jobs.get(role))
    def listener_ports(self,pid):return sorted(port for port,owner in self.ports.items() if owner==pid)
    def port_pids(self,port):return [] if port not in self.ports else [self.ports[port]]
    def event(self,role,pid,phase,**details):
        self.now+=.01
        row={'role':role,'pid':pid,'token':self.token,'phase':phase,'monotonic':self.now,
             'at':'fake-stdlib-time',**details}
        with (self.root/(role+'.events.jsonl')).open('ab') as stream:stream.write(json.dumps(row,sort_keys=True).encode()+b'\n')
        return row
    def identity(self,argv):
        self.next_pid+=1
        return {'pid':self.next_pid,'started_at':time.ctime(),'argv':list(argv)}
    def bootstrap(self,role,plist):
        self.calls.append(('bootstrap',role));ident=self.identity(self.fixture_host.argv(role));pid=ident['pid']
        self.jobs[role]=pid;self.processes[pid]=ident;port=40000+pid%10000;self.ports[port]=pid
        self.event(role,pid,'started',argv=ident['argv'][1:],cwd=str(self.root))
        ready={'pid':pid,'token':self.token,'role':role,'port':port,'protocol':'fixture_sentinel_only'}
        (self.root/(role+'.ready.json')).write_text(json.dumps(ready))
        if role=='provider':self.event(role,pid,'idle')
        else:
            if (self.root/'state.json').exists():self.state=r.read(self.root/'state.json')
            self.event(role,pid,'restored',state=copy.deepcopy(self.state))
        return SimpleNamespace(returncode=0,stdout='',stderr='')
    def debug(self,role,arguments,**kwargs):
        self.calls.append(('debug',role));return SimpleNamespace(returncode=self.debug_code,stdout='',stderr='')
    def signal(self,role,identity):
        self.calls.append(('signal',role));assert self.process(identity['pid'])==identity
        pid=identity['pid']
        if role=='provider':
            request_id='provider-'+str(pid)+'-fake'
            self.event(role,pid,'generation_admitted',request_id=request_id,ticks=11)
            self.event(role,pid,'stop_received',request_id=request_id);self.now+=6
            request={'token':self.token,'request_id':request_id,'handle':'fixture_astrid','ticks':11,'provider_pid':pid}
            r.write(self.root/('request.'+request_id+'.json'),request)
            self.event(role,pid,'checkin_sent',request_id=request_id)
            self.state['fixture_astrid']['ticks']+=11
            r.write(self.root/('ack.'+request_id+'.json'),{'token':self.token,'request_id':request_id,'ticks':11,'ok':True})
            self.event('reservoir',self.jobs['reservoir'],'checkin_applied',request_id=request_id,state=copy.deepcopy(self.state))
            self.event(role,pid,'checkin_acknowledged',request_id=request_id,ticks=11)
            self.event(role,pid,'drained',request_id=request_id)
        else:
            self.event(role,pid,'stop_received');self.now+=6
            self.event(role,pid,'final_snapshot_started',handles=sorted(self.state))
            (self.root/'state.json').write_text(json.dumps(self.state))
            for name,value in sorted(self.state.items()):self.event(role,pid,'handle_saved',name=name,value=value)
            self.event(role,pid,'final_snapshot_complete',state=copy.deepcopy(self.state));self.event(role,pid,'drained')
        self.processes.pop(pid)
        for port in self.listener_ports(pid):self.ports.pop(port)
        (self.root/(role+'.ready.json')).unlink(missing_ok=True)
        held=self.identity(self.fixture_host.hold_argv(role));guard=self.fixture_host.guard()
        self.jobs[role]=held['pid'];self.processes[held['pid']]=held
        directory=self.fixture_host.control(role)/'held';directory.mkdir(exist_ok=True,mode=0o700)
        r.write(directory/(str(held['pid'])+'.json'),{'schema':'model_environment_held_v1','pid':held['pid'],
            'started_at':held['started_at'],'hold':guard,'hold_only':True,'model_imported':False,'listener_opened':False,
            'launcher_sha256':h.sha(self.root/'model_environment.py')})
    def bootout(self,role):
        self.calls.append(('bootout',role));pid=self.jobs.pop(role);self.processes.pop(pid)
        return SimpleNamespace(returncode=0,stdout='',stderr='')


class HostTests(unittest.TestCase):
    def setUp(self):
        self.module_patch=patch.dict(sys.modules);self.module_patch.start();self.addCleanup(self.module_patch.stop)
        for name in MODULES:sys.modules.pop(name,None)
        self.forbidden=[]
        for name in ('subprocess.run','subprocess.Popen','os.kill','socket.socket'):
            guard=patch(name,side_effect=AssertionError('forbidden real IO: '+name));guard.start();self.addCleanup(guard.stop)
        self.core_binding={'root':str(CORE_ROOT),'artifacts':{str(p):h.sha(p) for p in CORE_ROOT.glob('*.py')}}
        self.core=load_core({'core':self.core_binding});self.token=uuid.uuid4().hex
        runs=h.HERE/'runs';runs.mkdir(exist_ok=True,mode=0o700)
        self.root=runs/('owned-recovery-'+self.token);self.root.mkdir(mode=0o700)
        self.addCleanup(lambda:shutil.rmtree(self.root))
        self.python=Path(sys.executable);binding={'uid':os.getuid(),'boot_uuid':'fake-boot','os':'stdlib-fake'}
        self.io=FakeIO(self.root,self.token,self.python,binding)
        # IO.host is the actual read-only method; keep the Host reference under
        # another attribute to avoid accidentally using a real system adapter.
        self.io.host_binding_method=self.io.host_binding
        self.io.host=self.io.host_binding_method
        artifacts={str(p):h.sha(p) for p in h.HERE.glob('*.py')}
        artifacts.update({str(p):h.sha(p) for p in (h.WORKER_SOURCE,h.HOLD_SOURCE)})
        for source in (h.WORKER_SOURCE,h.HOLD_SOURCE):shutil.copyfile(source,self.root/source.name)
        envspec=importlib.util.spec_from_file_location('fixture_test_hold',h.HOLD_SOURCE)
        env=importlib.util.module_from_spec(envspec);envspec.loader.exec_module(env)
        python_files={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in env.interpreter_paths(self.python)}
        targets={role:r.target(os.getuid(),self.token,role) for role in ('provider','reservoir','pin')}
        release={'schema':'owned_recovery_fixture_release_v1','approved':True,'scope':'owned_fixture_only',
            'production_qualified':False,'final_release_acceptance':False,'host_binding':binding,
            'python':str(self.python),'python_files':python_files,'artifacts':artifacts,'core':self.core_binding,
            'fixture':{'root':str(self.root),'token':self.token,'targets':targets}}
        release_ref=r.write(self.root/'test-release.json',release)
        owner={'schema':'owned_fixture_setup_owner_v1','token':self.token,'root':str(self.root),'uid':os.getuid(),
            'host_binding':binding,'coordinator_identity':self.io.executor,'release_ref':release_ref,'targets':targets,'python':str(self.python)}
        r.write(self.root/'setup-owner.json',owner)
        r.write(self.root/'fixture.owner.json',{'schema':'reservoir_maintenance_owned_fixture_v1',
            'token':self.token,'root':str(self.root),'uid':os.getuid()})
        for name,contents in (('reservoir-source.marker','old'),('reservoir-repair.marker','new'),('model-never-opened.sentinel','never')):
            (self.root/name).write_text(contents)
        self.host=h.FixtureHost.create(self.root,self.token,self.python,artifacts,core=self.core,io=self.io)
        self.io.fixture_host=self.host
        r.Ledger(self.root,'setup',{'owner_sha256':r.digest(owner),'release_sha256':release_ref['sha256']},create=True)
        for role in ('provider','reservoir'):(self.root/(role+'.plist')).write_bytes(plistlib.dumps(self.host.plist_settings(role)))
        # The fake IO needs a Host pointer; the public host() method stays an
        # inert binding getter. Real controls remain globally forbidden above.
        self.io.host=self.io.host_binding_method

    def setup_complete(self):
        from owned_fixture_setup import bootstrap_one,publish_complete
        for role in ('reservoir','provider'):
            bootstrap_one(self.root,self.host,role,mode='execute',executor=self.io.executor)
        m=self.core['m']
        plan={'schema':'reservoir_maintenance_plan_v1','transaction_id':self.token,'actor':'owned-fake-only',
            'pause_generation':1,'host_binding':self.host.binding,'input_hashes':self.host.inputs(),'protected':{},'timeout_s':90,
            'roles':{role:{'label':m.LABELS[role],'port':m.PORTS[role],
                           'identity':self.host.setup_observe(role)['process']} for role in m.ROLES}}
        spec=self.host.specification(plan);publish_complete(self.root,self.host,plan,spec)
        return m.Journal(self.root/'controller',plan=plan)

    def checkpoint(self,journal):
        report=self.host.recovery_inspection(journal)
        ledger=self.core['recovery_ledger'].read_ledger(self.root,self.host.plan)
        return self.core['recovery_phases'].checkpoint_from_inspection(self.host.spec,report,ledger)

    def step(self,journal,mode='execute'):
        checkpoint=self.checkpoint(journal)
        self.host.expect_recovery_action(checkpoint['action_id'])
        return self.core['production_recovery_driver'].resume_one(journal,self.host,
            expected=self.core['production_recovery_driver'].expected_heads(checkpoint),mode=mode)

    def reattach(self):
        self.io.executor['argv']=[str(self.python),'-B','owned-fake-reattached-executor']
        self.host=h.FixtureHost.attach(self.root,core=self.core,io=self.io)
        self.io.fixture_host=self.host
        return self.host

    def test_fresh_exact_controller_finishes_without_any_real_io(self):
        journal=self.setup_complete();result=self.core['m'].Controller(journal,self.host).run()
        self.assertEqual(result['status'],'verified');self.assertFalse(result['production_qualified'])
        self.assertEqual(r.read(self.root/'state.json')['fixture_astrid']['ticks'],11)
        self.assertTrue(self.checkpoint(journal)['completed'])
        self.assertEqual(self.io.calls.count(('signal','provider')),1)
        self.assertEqual(self.io.calls.count(('signal','reservoir')),1)

    def test_debug_result_interruption_adopts_without_rearming(self):
        journal=self.setup_complete()
        def interrupt(name,ref):
            if name=='host:debug_result':raise RuntimeError('injected after durable result')
        self.host.boundary_hook=interrupt
        with self.assertRaisesRegex(RuntimeError,'injected'):self.core['m'].Controller(journal,self.host).run()
        original=copy.deepcopy(journal.events);self.host.boundary_hook=None
        self.reattach()
        self.assertEqual(self.checkpoint(journal)['action_id'],'arm_next_spawn:provider')
        self.step(journal,'reconcile')
        self.assertEqual(self.io.calls.count(('debug','provider')),1)
        self.assertEqual(self.core['m'].Journal(journal.root).events,original)
        self.assertEqual(self.step(journal)['action_id'],'signal_stop:provider')

    def test_unknown_debug_with_live_original_refuses_replay_and_adoption(self):
        journal=self.setup_complete();self.io.debug_code=5
        with self.assertRaisesRegex(ValueError,'unknown/nonzero'):self.core['m'].Controller(journal,self.host).run()
        with self.assertRaises(Exception):self.step(journal)
        with self.assertRaisesRegex(ValueError,'unknown command'):self.step(journal,'reconcile')
        self.assertEqual(self.io.calls.count(('debug','provider')),1)
        self.assertNotIn(('signal','provider'),self.io.calls)

    def test_changed_host_and_foreign_argv_are_refused(self):
        self.setup_complete();self.io.binding['boot_uuid']='other'
        with self.assertRaisesRegex(ValueError,'host identity'):self.host.inspect()
        self.io.binding['boot_uuid']='fake-boot'
        pid=self.io.jobs['provider'];self.io.processes[pid]['argv'].append('foreign')
        with self.assertRaises(ValueError):self.host.inspect_role('provider')

    def test_foreign_old_sentinel_listener_prevents_absence(self):
        self.setup_complete();role='provider';old=self.io.jobs.pop(role);self.io.processes.pop(old)
        port=self.host.ports[role];self.io.ports[port]=999999
        with self.assertRaisesRegex(ValueError,'not absent'):self.host.absent(role)

    def test_no_host_mutation_outside_shared_session(self):
        self.setup_complete();before=list(self.io.calls)
        with self.assertRaises(Exception):self.host.create_hold('provider',self.host.guard())
        self.assertEqual(self.io.calls,before)

    def test_wrong_planned_action_blocks_before_recovery_claim(self):
        journal=self.setup_complete();checkpoint=self.checkpoint(journal)
        self.host.expect_recovery_action('create_hold:provider')
        before=list(self.io.calls)
        with self.assertRaisesRegex(ValueError,'planned fixture action'):
            self.core['production_recovery_driver'].resume_one(journal,self.host,
                expected=self.core['production_recovery_driver'].expected_heads(checkpoint))
        self.assertEqual(self.io.calls,before);self.assertFalse((self.root/'recovery').exists())

    def test_all_one_step_actions_then_separate_cleanup_preserve_main(self):
        journal=self.setup_complete()
        for _ in range(35):
            checkpoint=self.checkpoint(journal)
            if checkpoint['completed']:break
            self.step(journal,'reconcile' if checkpoint['requires_reconciliation'] else 'execute')
        self.assertTrue(self.checkpoint(journal)['completed'])
        self.assertEqual(journal.events,[])
        from owned_fixture_cleanup import begin,step,ACTIONS
        context=begin(self.host,self.core)
        main={str(path):r.reference(path) for path in self.root.glob('host-operation-*.json')}
        for action in ACTIONS:
            self.reattach()
            ledger=r.Ledger(self.root,'cleanup',{'context_sha256':r.digest(context)})
            result=step(self.host,self.core,expected_head=ledger.head,action=action,mode='execute')
        self.assertTrue(result['complete']);self.assertEqual(r.read(self.root/'state.json')['fixture_astrid']['ticks'],22)
        self.assertEqual(main,{str(path):r.reference(path) for path in self.root.glob('host-operation-*.json')})
        self.assertEqual(self.io.calls.count(('signal','provider')),2)
        self.assertEqual(self.io.calls.count(('signal','reservoir')),2)

    def test_witness_effect_adoption_does_not_overwrite_evidence(self):
        journal=self.setup_complete()
        def interrupt(name,ref):
            if name=='witness:provider_drain':raise RuntimeError('after witness')
        self.host.boundary_hook=interrupt
        with self.assertRaisesRegex(RuntimeError,'after witness'):self.core['m'].Controller(journal,self.host).run()
        path=self.root/'provider_drain-witness.json';saved=r.reference(path)
        self.host.boundary_hook=None;self.step(journal,'reconcile')
        self.assertEqual(r.reference(path),saved);self.assertEqual(self.io.calls.count(('signal','provider')),1)

    def test_install_effect_adoption_and_bootstrap_result_adoption(self):
        journal=self.setup_complete()
        def interrupt(name,ref):
            if name=='host:install_completed':raise RuntimeError('after install')
        self.host.boundary_hook=interrupt
        with self.assertRaisesRegex(RuntimeError,'after install'):self.core['m'].Controller(journal,self.host).run()
        self.host.boundary_hook=None;self.step(journal,'reconcile')
        def bootstrap_interrupt(name,ref):
            if name=='host:bootstrap_ready':raise RuntimeError('after bootstrap')
        self.host.boundary_hook=bootstrap_interrupt
        with self.assertRaisesRegex(RuntimeError,'after bootstrap'):self.step(journal)
        self.host.boundary_hook=None;self.step(journal,'reconcile')
        self.assertEqual(self.io.calls.count(('bootstrap','reservoir')),2)
        self.assertEqual(self.checkpoint(journal)['action_id'],'witness:startup')

    def test_partial_witness_is_not_reproduced(self):
        journal=self.setup_complete()
        with patch('owned_fixture_retained.make_witness',side_effect=RuntimeError('before evidence')):
            with self.assertRaisesRegex(RuntimeError,'before evidence'):
                self.core['m'].Controller(journal,self.host).run()
        self.assertTrue((self.root/'provider_drain-witness-intent.json').exists())
        self.assertTrue(self.checkpoint(journal)['requires_reconciliation'])
        before=list(self.io.calls)
        with self.assertRaises(Exception):self.step(journal)
        with self.assertRaises(FileNotFoundError):self.step(journal,'reconcile')
        self.assertEqual(before,self.io.calls)

    def test_stale_owned_ready_marker_is_not_accepted_as_new_readiness(self):
        journal=self.setup_complete()
        def interrupt(name,ref):
            if name=='host:install_completed':raise RuntimeError('after install')
        self.host.boundary_hook=interrupt
        with self.assertRaises(RuntimeError):self.core['m'].Controller(journal,self.host).run()
        self.host.boundary_hook=None;self.step(journal,'reconcile')
        bootstrap=self.io.bootstrap;sleep=self.io.sleep;pending={}
        def delayed(role,plist):
            result=bootstrap(role,plist);path=self.root/(role+'.ready.json');pending['path']=path;pending['value']=r.read(path)
            prior={'pid':self.host.plan['roles'][role]['identity']['pid'],'token':self.token,'role':role,
                   'port':self.host.spec['anchors'][role]['sentinel_port'],'protocol':'fixture_sentinel_only'}
            path.write_text(json.dumps(prior));return result
        def publish(seconds):
            sleep(seconds)
            if pending:pending.pop('path').write_text(json.dumps(pending.pop('value')))
        self.io.bootstrap=delayed;self.io.sleep=publish
        self.assertEqual(self.step(journal)['action_id'],'bootstrap_original:reservoir')
        self.assertEqual(self.io.calls.count(('bootstrap','reservoir')),2)

    def test_setup_mutation_requires_held_setup_lock(self):
        owner=self.host.owner;ledger=r.Ledger(self.root,'setup',{'owner_sha256':r.digest(owner),
            'release_sha256':owner['release_ref']['sha256']})
        claim=ledger.append('claim','bootstrap:reservoir',target=self.host.target('reservoir'),
            plist_ref=r.reference(self.root/'reservoir.plist'),expected_argv=self.host.argv('reservoir'),executor_identity=self.io.executor)
        with self.assertRaisesRegex(ValueError,'setup'):
            self.host.setup_bootstrap('reservoir',claim)
        self.assertEqual(self.io.calls,[])

    def test_unrelated_role_replacement_blocks_cleanup_before_mutation(self):
        journal=self.setup_complete();self.core['m'].Controller(journal,self.host).run()
        from owned_fixture_cleanup import begin,step
        context=begin(self.host,self.core)
        old=self.io.jobs.pop('reservoir');self.io.processes.pop(old)
        for port in self.io.listener_ports(old):self.io.ports.pop(port)
        self.io.bootstrap('reservoir',self.root/'reservoir.plist')
        before=list(self.io.calls)
        ledger=r.Ledger(self.root,'cleanup',{'context_sha256':r.digest(context)})
        with self.assertRaisesRegex(ValueError,'cleanup original or expected drain role changed'):
            step(self.host,self.core,expected_head=ledger.head,action='create_hold:provider',mode='execute')
        self.assertEqual(before,self.io.calls)
        self.assertFalse((self.host.control('provider')/'hold.json').exists())

    def test_ack_before_applied_event_waits_without_replaying_checkin(self):
        journal=self.setup_complete();event=self.io.event;sleep=self.io.sleep;pending=[]
        def delayed(role,pid,phase,**details):
            if phase=='checkin_applied':pending.append((role,pid,phase,details));return None
            return event(role,pid,phase,**details)
        def publish(seconds):
            sleep(seconds)
            while pending:
                role,pid,phase,details=pending.pop(0);event(role,pid,phase,**details)
        self.io.event=delayed;self.io.sleep=publish
        result=self.core['m'].Controller(journal,self.host).run()
        self.assertEqual(result['status'],'verified')
        self.assertEqual(self.io.calls.count(('signal','provider')),1)
        self.assertEqual(len(list(self.root.glob('request.*.json'))),1)

    def test_duplicate_applied_event_refuses_without_wait_or_replay(self):
        journal=self.setup_complete();event=self.io.event
        def duplicate(role,pid,phase,**details):
            value=event(role,pid,phase,**details)
            if phase=='checkin_applied':event(role,pid,phase,**details)
            return value
        self.io.event=duplicate
        with self.assertRaisesRegex(ValueError,'duplicate exact check-in'):
            self.core['m'].Controller(journal,self.host).run()
        self.assertEqual(self.io.calls.count(('signal','provider')),1)
        self.assertNotIn(('signal','reservoir'),self.io.calls)

    def test_cleanup_signalled_publication_gap_reaches_bounded_holder_observation(self):
        journal=self.setup_complete();self.core['m'].Controller(journal,self.host).run()
        from owned_fixture_cleanup import begin,step
        context=begin(self.host,self.core)
        for operation in ('create_hold','arm_next_spawn','signal_stop'):
            ledger=r.Ledger(self.root,'cleanup',{'context_sha256':r.digest(context)})
            step(self.host,self.core,expected_head=ledger.head,action=operation+':provider',mode='execute')
        job=self.io.job;gap=[True]
        def transient(role):
            if role=='provider' and gap:gap.pop();return True,None
            return job(role)
        self.io.job=transient
        ledger=r.Ledger(self.root,'cleanup',{'context_sha256':r.digest(context)})
        result=step(self.host,self.core,expected_head=ledger.head,action='held_verified:provider',mode='execute')
        self.assertNotEqual(result['result']['identity'],context['main_identities']['provider'])
        self.assertEqual(self.io.calls.count(('signal','provider')),2)


if __name__=='__main__':unittest.main()
