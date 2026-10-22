"""Strict two-job owned Host for the unchanged04 lifecycle interfaces.

No work on import. The coordinator must supply verified core modules, explicit
IO and immutable setup ownership. This class cannot target production services.
"""
from __future__ import annotations
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import time

import owned_fixture_records as r

HERE = Path(__file__).resolve().parent
WORKER_SOURCE = HERE.parent/'candidate-v2/maintenance_fixture_worker.py'
HOLD_SOURCE = Path('/Users/v/other/astrid/scripts/model_environment.py')
ROLES = ('provider','reservoir')
PAIRS = {'hold_intent':'hold_created','debug_intent':'debug_result',
         'signal_intent':'signal_sent','held_bootout_intent':'held_job_absent',
         'install_intent':'install_completed','bootstrap_intent':'bootstrap_ready',
         'hold_release_intent':'hold_released'}


def sha(path):
    return hashlib.sha256(r.raw(path)).hexdigest()


def raw_result(value):
    return {'returncode':value.returncode,'stdout':getattr(value,'stdout',None),
            'stderr':getattr(value,'stderr',None)}


class FixtureHost:
    scope = 'owned_fixture'

    @classmethod
    def create(cls,root,token,executable,artifacts,*,core,io,sudo_debug=False,boundary_hook=None):
        self=cls()
        self.root=r.directory(root);self.token=token;self.executable=Path(executable)
        self.artifacts=dict(artifacts);self.core=core;self.m=core['m'];self.io=io
        self.sudo_debug=sudo_debug;self.boundary_hook=boundary_hook
        r.require(io is not None and self.executable.is_absolute(),'explicit IO/executable required')
        self.owner=r.read(self.root/'setup-owner.json');self.binding=self.owner['host_binding'];self.host_binding=self.binding
        self.uid=self.owner['uid'];self.plan=None;self.spec=None;self.guards={};self.armed=set();self.signalled=set()
        self.receipts={};self.startups={};self.ports={};self.known_ports={role:set() for role in ROLES}
        self.repaired=False;self.cleanup=None;self.cleanup_authorizer=None
        self._validated=None;self.expected_recovery_action=None;self.check_owned()
        spec=importlib.util.spec_from_file_location('owned_hold_'+token,self.root/'model_environment.py')
        self.env=importlib.util.module_from_spec(spec);spec.loader.exec_module(self.env)
        self.python_files={str(p):hashlib.sha256(Path(p).read_bytes()).hexdigest()
                           for p in self.env.interpreter_paths(self.executable)}
        self.check_owned()
        return self

    @classmethod
    def attach(cls,root,*,core,io,sudo_debug=False,boundary_hook=None):
        root=r.directory(root);complete=r.read(root/'setup-complete.json')
        spec=r.check_reference(complete['spec_ref']);plan=r.check_reference(complete['plan_ref'])
        r.require(spec['plan']==plan and spec['transaction_root']==str(root),'retained preparation differs')
        self=cls.create(root,plan['transaction_id'],spec['executable'],spec['artifacts'],
                        core=core,io=io,sudo_debug=sudo_debug,boundary_hook=boundary_hook)
        self.bind_plan(plan,spec)
        return self

    def check_owned(self):
        r.directory(self.root)
        r.require(os.geteuid()!=0 and self.uid==os.getuid() and self.uid>0,'ordinary owning user required')
        r.require(r.read(self.root/'fixture.owner.json')=={
            'schema':'reservoir_maintenance_owned_fixture_v1','token':self.token,
            'root':str(self.root),'uid':self.uid},'worker owner changed')
        r.require(self.owner.get('schema')=='owned_fixture_setup_owner_v1'
                  and self.owner['root']==str(self.root) and self.owner['token']==self.token
                  and r.read(self.root/'setup-owner.json')==self.owner,'setup owner changed')
        r.require(self.root.parent==HERE/'runs' and self.root.name=='owned-recovery-'+self.token,
                  'fixture root is outside exact private run tree')
        r.require(self.owner['targets']=={role:self.target(role) for role in (*ROLES,'pin')},'fixture targets differ')
        release=r.check_reference(self.owner['release_ref'])
        r.require(release.get('schema')=='owned_recovery_fixture_release_v2'
                  and release.get('approved') is True and release.get('scope')=='owned_fixture_only'
                  and release.get('production_qualified') is False and release.get('final_release_acceptance') is False
                  and release.get('host_binding')==self.binding and release.get('python')==str(self.executable)
                  and release.get('artifacts')==self.artifacts
                  and release.get('fixture')=={'root':str(self.root),'token':self.token,'targets':self.owner['targets']},
                  'exact preparatory owned release differs')
        from owned_recovery_core import load_core
        r.require(load_core(release)==self.core,'fixture core module identity differs')
        if hasattr(self,'python_files'):
            r.require(release.get('python_files')==self.python_files,'interpreter family changed')
            for path,wanted in self.python_files.items():
                r.require(hashlib.sha256(Path(path).read_bytes()).hexdigest()==wanted,'interpreter bytes changed')
        for path,digest in self.artifacts.items():r.require(sha(path)==digest,'reviewed fixture artifact changed: '+path)
        for source in (WORKER_SOURCE,HOLD_SOURCE):
            r.require(str(source) in self.artifacts and sha(self.root/source.name)==self.artifacts[str(source)],
                      'private copied source differs')
        r.require(self.io.host()==self.binding,'host identity changed')

    def target(self,role):return r.target(self.uid,self.token,role)
    def control(self,role):
        r.require(role in ROLES,'unknown fixture role');return self.root/('hold-'+role)
    def argv(self,role):
        r.require(role in ROLES,'unknown fixture role')
        return [str(self.executable),str(self.root/'maintenance_fixture_worker.py'),
                '--root',str(self.root),'--token',self.token,'--role',role,'--drain-seconds','6.0']
    def hold_argv(self,role,guard=None):
        guard=guard or self.guards.get(role) or self.guard()
        return [str(self.executable),str(self.root/'model_environment.py'),'launch',
                '--control-root',str(self.control(role)),'--hold-only',guard['transaction_id']]
    def guard(self):
        if self.cleanup:return self.cleanup['guard']
        r.require(self.plan is not None,'main plan unavailable')
        return {'schema':'model_environment_hold_v1','transaction_id':self.token,'actor':self.plan['actor']}
    def plist_settings(self,role):
        return {'Label':self.target(role).split('/')[-1],'ProgramArguments':self.argv(role),
                'WorkingDirectory':str(self.root),'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':1,'ExitTimeOut':5,
                'StandardOutPath':str(self.root/(role+'.stdout')),'StandardErrorPath':str(self.root/(role+'.stderr'))}
    def executor_identity(self):
        value=self.io.process(os.getpid());r.identity(value)
        r.require(value['pid']==os.getpid(),'executor PID differs');return value
    def monotonic(self):return self.io.monotonic()
    def sleep(self,seconds):return self.io.sleep(seconds)

    def wait(self,predicate,label,timeout=90):
        end=self.monotonic()+timeout
        while True:
            value=predicate()
            if value:return value
            r.require(self.monotonic()<end,label+' timed out; retain jobs/evidence, no escalation')
            self.sleep(.1)

    def event(self,operation,*,setup=False,**details):
        directory=self.root/'setup-host' if setup else self.root/'cleanup' if self.cleanup else self.root
        if setup and not directory.exists():directory.mkdir(mode=0o700)
        paths=sorted(directory.glob('host-operation-*.json'))
        r.require(len(paths)<500,'fixture operation bound exceeded')
        for index,path in enumerate(paths,1):r.require(path.name==f'host-operation-{index:05d}.json','Host history gap')
        from owned_fixture_retained import action_authority
        authority=None if setup else action_authority(self,operation,details.get('role'))
        row={'operation':operation,'monotonic':self.monotonic(),'wall_time_ns':time.time_ns(),**details}
        if authority is not None:row['authority']=authority
        ref=r.write(directory/f'host-operation-{len(paths)+1:05d}.json',row)
        if self.boundary_hook:self.boundary_hook(('setup:' if setup else 'cleanup:' if self.cleanup else 'host:')+operation,ref)
        return row

    def events(self,role,pid=None,*,complete=False):
        path=self.root/(role+'.events.jsonl')
        if not path.exists():return []
        data=r.raw(path)
        if complete:r.require(not data or data.endswith(b'\n'),'partial stopped worker event')
        data=data[:data.rfind(b'\n')+1]
        rows=[json.loads(line) for line in data.splitlines()]
        r.require(all(row.get('token')==self.token and row.get('role')==role for row in rows),'foreign worker event')
        return [row for row in rows if pid is None or row.get('pid')==pid]

    def inspect_role(self,role):
        r.require(role in ROLES,'unknown fixture role')
        loaded,pid=self.io.job(role);process=self.io.process(pid)
        for _ in range(20):
            if pid is None or process is not None:break
            new_loaded,new_pid=self.io.job(role)
            if new_pid==pid:self.sleep(.01);continue
            loaded,pid=new_loaded,new_pid;process=self.io.process(pid)
        r.require(pid is None or process is not None,'disappeared PID remains published')
        view={'target':self.target(role),'job_loaded':loaded,'process':process,'listener_pids':[],'held':None}
        self.remember_ports(role)
        view['listener_pids']=sorted({owner for port in self.known_ports[role] for owner in self.io.port_pids(port)})
        if process is None:return view
        ports=self.io.listener_ports(pid)
        if self.env.command_matches(process['argv'],self.argv(role)):
            ready=self.root/(role+'.ready.json')
            if ready.exists():
                record=r.read(ready)
                r.require(record.get('token')==self.token and record.get('role')==role
                          and record.get('protocol')=='fixture_sentinel_only','foreign ready record')
                if record.get('pid')!=pid:
                    from owned_fixture_retained import prior_ready_during_bootstrap
                    r.require(prior_ready_during_bootstrap(self,role,process,record), 'unproved stale ready marker')
                    r.require(len(ports)<=1 and not view['listener_pids'],'foreign listener during startup')
                    return view
                draining=(role,pid,process['started_at']) in self.signalled
                r.require(ports==[record.get('port')] or draining and ports==[],'worker sentinel ownership differs')
                self.ports[role]=record['port'];self.known_ports[role].add(record['port'])
                view['listener_pids']=sorted({owner for port in self.known_ports[role] for owner in self.io.port_pids(port)})
                r.require(view['listener_pids']==([pid] if ports else []),'foreign sentinel listener')
            else:r.require(len(ports)<=1,'worker has extra listeners')
            return view
        guard_path=self.control(role)/'hold.json'
        guard=r.read(guard_path) if guard_path.exists() else None
        r.require(guard is not None and guard in (self.guard(),self.guards.get(role)),'foreign/missing holder guard')
        r.require(self.env.command_matches(process['argv'],self.hold_argv(role,guard)),'unexpected replacement command')
        r.require(not ports and not view['listener_pids'],'holder has a listener')
        receipt=self.control(role)/'held'/(str(pid)+'.json')
        if not receipt.exists():return dict(view,process=None)
        record=r.read(receipt)
        r.require(record.get('schema')=='model_environment_held_v1' and record.get('pid')==pid
                  and record.get('hold')==guard and record.get('started_at')==process['started_at']
                  and record.get('hold_only') is True and record.get('model_imported') is False
                  and record.get('listener_opened') is False
                  and record.get('launcher_sha256')==sha(self.root/'model_environment.py'),'holder receipt differs')
        self.env.assert_hold(self.control(role),guard)
        view['held']={'guard':guard,'identity':process,'hold_only':True,'stdlib_only':True,'state_touched':False}
        return view

    def setup_observe(self,role):
        view=self.inspect_role(role);process=view['process']
        phases=[row['phase'] for row in self.events(role,process['pid'])] if process else []
        view['ready']=bool(process and not view['held'] and view['listener_pids']==[process['pid']]
                           and ('idle' if role=='provider' else 'restored') in phases)
        return view

    def setup_claim(self,role,claim):
        from owned_fixture_setup import require_setup_active
        require_setup_active(self.root)
        self.check_owned();r.require(self.plan is None,'setup is complete; no setup replay')
        self.validate_setup_owner(self.owner)
        binding={'owner_sha256':r.digest(self.owner),'release_sha256':self.owner['release_ref']['sha256']}
        ledger=r.Ledger(self.root,'setup',binding)
        r.require(ledger.rows and ledger.rows[-1]==claim and claim.get('type')=='claim'
                  and claim.get('action')=='bootstrap:'+role and claim.get('target')==self.target(role)
                  and claim.get('expected_argv')==self.argv(role)
                  and claim.get('executor_identity')==self.executor_identity(),'setup claim differs')
        r.require(claim.get('plist_ref')==r.reference(self.root/(role+'.plist')),'setup plist claim differs')
        r.require(plistlib.loads(r.raw(self.root/(role+'.plist')))==self.plist_settings(role),'private plist differs')
        if self.executor_identity()!=self.owner['coordinator_identity']:
            successor=r.read(self.root/'setup-successor.json')
            r.require(successor['permitted_action']==claim['action']
                      and successor['expected_setup_head']==claim['previous'],'successor action/head differs')

    def setup_bootstrap(self,role,claim):
        self.setup_claim(role,claim);view=self.inspect_role(role)
        r.require(not view['job_loaded'] and view['process'] is None and not view['listener_pids'],'setup target already exists')
        self.event('bootstrap_intent',setup=True,role=role,claim_sha256=r.digest(claim),expected_argv=self.argv(role))
        result=self.io.bootstrap(role,self.root/(role+'.plist'))
        self.event('bootstrap_command_returned',setup=True,role=role,**raw_result(result))
        r.require(result.returncode==0,'setup bootstrap unsuccessful; never retry')
        view=self.wait(lambda:self._ready(role),'initial same-PID worker readiness')
        self.event('bootstrap_ready',setup=True,role=role,identity=view['process'],sentinel_port=self.ports[role])
        return {'identity':view['process'],'sentinel_port':self.ports[role],'ready':True,'scope':'owned_fixture_only'}

    def setup_reconcile(self,role,original_claim):
        from owned_fixture_setup import require_setup_active
        require_setup_active(self.root)
        self.check_owned();r.require(self.plan is None,'setup already complete')
        from owned_fixture_retained import setup_effect
        result,evidence=setup_effect(self,role,original_claim)
        return {'passed':True,'action':'bootstrap:'+role,'original_claim_sha256':r.digest(original_claim),
                'result':result,'evidence':evidence}

    def _ready(self,role):
        view=self.setup_observe(role);return view if view['ready'] else None

    def cursor(self,role):
        path=self.root/(role+'.events.jsonl');data=r.raw(path)
        r.require(data.endswith(b'\n'),'incomplete initial event boundary')
        info=path.stat()
        return {'path':str(path),'start_offset':len(data),'prefix_sha256':hashlib.sha256(data).hexdigest(),
                'device':info.st_dev,'inode':info.st_ino}

    def inputs(self):
        return {'provider_source':sha(self.root/'maintenance_fixture_worker.py'),
                'provider_settings':sha(self.root/'provider.plist'),'provider_packages':r.digest('owned stdlib only'),
                'model_snapshot':sha(self.root/'model-never-opened.sentinel'),
                'reservoir_source':sha(self.root/'reservoir-source.marker'),
                'reservoir_candidate':sha(self.root/'reservoir-repair.marker'),
                'reservoir_settings':sha(self.root/'reservoir.plist'),
                'interpreter':r.digest(self.python_files),'helpers':r.digest(self.artifacts),
                'fixture':self.artifacts[str(Path(__file__).resolve())]}

    def specification(self,plan):
        self.check_owned();self.m.validate_plan(plan)
        r.require(plan['transaction_id']==self.token and plan['input_hashes']==self.inputs(),'planned fixture inputs differ')
        anchors={role:{'identity':copy.deepcopy(plan['roles'][role]['identity']),'log':self.cursor(role),
                       'sentinel_port':self.ports[role]} for role in ROLES}
        return {'schema':'owned_fixture_preparation_v1','plan':copy.deepcopy(plan),'transaction_root':str(self.root),
                'execution_scope':'owned_fixture','executable':str(self.executable),'artifacts':self.artifacts,
                'original_identities':{role:copy.deepcopy(plan['roles'][role]['identity']) for role in ROLES},
                'provider_restart_argv':plan['roles']['provider']['identity']['argv'],'anchors':anchors,
                'python_files':self.python_files,'production_qualified':False}

    def bind_plan(self,plan,spec):
        self.check_owned();self.m.validate_plan(plan)
        complete=r.read(self.root/'setup-complete.json')
        r.require(r.check_reference(complete['plan_ref'])==plan and r.check_reference(complete['spec_ref'])==spec,
                  'setup completion does not bind exact plan/spec')
        r.require(spec['schema']=='owned_fixture_preparation_v1' and spec['plan']==plan
                  and spec['transaction_root']==str(self.root) and spec['execution_scope']=='owned_fixture'
                  and spec['artifacts']==self.artifacts and spec['python_files']==self.python_files
                  and spec['executable']==str(self.executable) and plan['host_binding']==self.binding
                  and plan['transaction_id']==self.token,'retained fixture preparation differs')
        r.require(plan['protected']=={} and spec['production_qualified'] is False
                  and complete['identities']==spec['original_identities']=={
                      role:plan['roles'][role]['identity'] for role in ROLES},'fixture identity/scope differs')
        self.plan=copy.deepcopy(plan);self.spec=copy.deepcopy(spec);self.anchors=copy.deepcopy(spec['anchors'])
        for role in ROLES:self.remember_ports(role)

    def _allow(self,operation,role=None):
        self.check_owned();r.require(self.plan is not None,'setup is not complete')
        self.core['transaction_execution'].require_active(self.root,self.plan)
        if self.cleanup:
            r.require(self.cleanup_authorizer is not None,'cleanup action authorization absent')
            self.cleanup_authorizer(operation,role)

    def inspect(self):
        self.check_owned()
        return {'host_binding':self.binding,'paused':True,'pause_generation':self.plan['pause_generation'],
                'pause_actor':self.plan['actor'],'lease':None,'protected':{},'input_hashes':self.inputs(),
                'fixture_mode_authorized':True,'originals_preserved':True,'rollback_software_ready':True,
                'roles':{role:self.inspect_role(role) for role in ROLES},'ownership_scope':'owned fixture marker only'}

    def create_hold(self,role,guard):
        self._allow('create_hold',role);r.require(guard==self.guard(),'unexpected guard owner')
        self.event('hold_intent',role=role,guard=guard)
        result=self.env.create_hold(self.control(role),guard['transaction_id'],guard['actor'])
        r.require(result==guard,'created guard differs');self.guards[role]=guard
        self.event('hold_created',role=role,guard=guard);return {'guard':guard}

    def arm_next_spawn(self,role,guard,expected):
        self._allow('arm_next_spawn',role);r.require(guard==self.guard(),'arm guard differs')
        self.env.assert_hold(self.control(role),guard)
        view=self.inspect_role(role);r.require(view['process']==expected and not view['held'],'worker changed before arm')
        key=(role,expected['pid'],expected['started_at']);r.require(key not in self.armed,'arm already attempted')
        args=['--program',str(self.executable),'--',*self.hold_argv(role,guard)[1:]]
        self.event('debug_intent',role=role,identity=expected,arguments=args,guard=guard,sudo_debug=self.sudo_debug)
        self.armed.add(key)
        result=self.io.debug(role,args,sudo_debug=self.sudo_debug)
        self.event('debug_result',role=role,**raw_result(result))
        r.require(result.returncode==0,'unknown/nonzero debug; no signal or retry')
        return {'armed':True}

    def signal_stop(self,role,expected):
        self._allow('signal_stop',role);key=(role,expected['pid'],expected['started_at'])
        r.require(key not in self.signalled,'signal already attempted')
        view=self.inspect_role(role);r.require(view['process']==expected and not view['held'],'signal identity changed')
        from owned_fixture_retained import latest_pair
        pair=latest_pair(self,'debug_intent',role)
        r.require(pair and pair['result'] and pair['result'].get('returncode')==0
                  and pair['intent']['identity']==expected,'positive exact debug result required')
        self.event('signal_intent',role=role,identity=expected,signal='SIGTERM')
        self.signalled.add(key);self.io.signal(role,expected)
        self.event('signal_sent',role=role,identity=expected,signal='SIGTERM')
        return {'signal':'SIGTERM','identity':expected,'force_used':False}

    def remove_held(self,role,expected,guard):
        self._allow('remove_held',role);view=self.inspect_role(role)
        r.require(view['process']==expected and view['held'] and view['held']['guard']==guard
                  and not view['listener_pids'],'only exact no-listener holder may be removed')
        self.event('held_bootout_intent',role=role,identity=expected)
        result=self.io.bootout(role);r.require(result.returncode==0,'held removal outcome unknown')
        self.wait(lambda:not self.inspect_role(role)['job_loaded'] and self.io.process(expected['pid']) is None,
                  'held job/process absence',15)
        self.event('held_job_absent',role=role,identity=expected);return {'removed_identity':expected}

    def collect_witness(self,kind,context):
        self._allow('witness',kind)
        from owned_fixture_retained import make_witness
        session=self.core['transaction_execution'].require_active(self.root,self.plan)
        r.write(self.root/(kind+'-witness-intent.json'),{'kind':kind,'context':context,'executor_session':session.begin,
            'controller_head_sha256':self.m.Journal(self.root/'controller').previous,
            'recovery_head_sha256':self.core['recovery_ledger'].read_ledger(self.root,self.plan)['head_sha256']})
        value=make_witness(self,kind,context,produce=True)
        r.write(self.root/(kind+'-witness.json'),value);self.receipts[kind]=value
        if self.boundary_hook:self.boundary_hook('witness:'+kind,r.reference(self.root/(kind+'-witness.json')))
        return value

    def install_repair(self,context):
        self._allow('install_repair');self.absent('provider');self.absent('reservoir')
        r.require(self.receipts.get('private_restore',{}).get('passed') is True,'private fixture restore missing')
        source=self.root/'reservoir-repair.marker';target=self.root/'reservoir-source.marker'
        self.event('install_intent',source_sha256=sha(source),target=str(target))
        data=r.raw(source);temporary=self.root/'reservoir-source.next'
        fd=os.open(temporary,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
        with os.fdopen(fd,'wb') as out:out.write(data);out.flush();os.fsync(out.fileno())
        os.replace(temporary,target);r.sync(self.root);self.repaired=True
        self.event('install_completed',source_sha256=sha(target));return {'source_sha256':sha(target)}

    def bootstrap_original(self,role,context):
        self._allow('bootstrap_original',role);self.absent(role)
        r.require(plistlib.loads(r.raw(self.root/(role+'.plist')))==self.plist_settings(role),'bootstrap plist differs')
        anchor=self.cursor(role)
        self.event('bootstrap_intent',role=role,log_anchor=anchor,expected_argv=self.argv(role),
                   original_rollback_plist_sha256=sha(self.root/(role+'.plist')),
                   applied_plist_sha256=sha(self.root/(role+'.plist')))
        result=self.io.bootstrap(role,self.root/(role+'.plist'))
        self.event('bootstrap_command_returned',role=role,**raw_result(result))
        r.require(result.returncode==0,'bootstrap outcome unsuccessful; no retry')
        view=self.wait(lambda:self._ready(role),'replacement same-PID readiness')
        self.startups[role]={'identity':view['process'],'log_anchor':anchor}
        self.event('bootstrap_ready',role=role,identity=view['process'],sentinel_port=self.ports[role]);return {'identity':view['process']}

    def release_hold(self,role,guard):
        self._allow('release_hold',role);r.require(guard==self.guard(),'release guard differs')
        self.event('hold_release_intent',role=role,guard=guard)
        self.env.release_hold(self.control(role),guard);self.guards.pop(role,None)
        self.event('hold_released',role=role,guard=guard);return {'released':True}

    def absent(self,role):
        view=self.inspect_role(role)
        r.require(not view['job_loaded'] and view['process'] is None and not view['listener_pids'],'role is not absent')

    def recovery_inspection(self,journal):
        from owned_fixture_retained import inspect
        return inspect(self,journal)
    def expect_recovery_action(self,action_id):
        r.require(action_id in {row['id'] for row in self.core['recovery_phases'].actions(production=False)},
                  'unknown planned owned recovery action')
        self.expected_recovery_action=action_id
    def revalidate_recovery(self,checkpoint,inspection):
        from owned_fixture_retained import revalidate
        return revalidate(self,checkpoint,inspection)
    def reconcile_recovery(self,action_id,checkpoint,inspection):
        from owned_fixture_retained import reconcile
        return reconcile(self,action_id,checkpoint,inspection)
    def enable_cleanup(self,context,authorizer):
        from owned_fixture_cleanup_host import enable_cleanup
        return enable_cleanup(self,context,authorizer)
    def cleanup_witness(self,role,context):
        from owned_fixture_cleanup_host import cleanup_witness
        return cleanup_witness(self,role,context,produce=True)
    def reconcile_cleanup(self,action_id,context,original_claim):
        from owned_fixture_cleanup_host import reconcile_cleanup
        return reconcile_cleanup(self,action_id,context,original_claim)

    def validate_setup_owner(self,owner,require_executor=True):
        self.check_owned();r.require(owner==self.owner,'different setup owner')
        if not require_executor:return
        executor=self.executor_identity()
        if executor==owner['coordinator_identity']:return
        path=self.root/'setup-successor.json'
        # A new observer may reconcile a retained attempt. It cannot execute an
        # unattempted launch without the explicit single-action successor file.
        if not path.exists():
            ledger=r.Ledger(self.root,'setup',{'owner_sha256':r.digest(owner),'release_sha256':owner['release_ref']['sha256']})
            r.require(ledger.rows and any(ledger.outstanding(row['action']) for row in ledger.rows)
                      and self.io.process(owner['coordinator_identity']['pid']) is None,
                      'setup executor has no successor or unresolved adoption')
            return
        successor=r.read(path);release=r.check_reference(owner['release_ref'])
        r.require(release.get('setup_successor_policy')=={'max_successors':1,
                  'allowed_actions':['bootstrap:reservoir','bootstrap:provider'],'no_attempt_replay':True}
                  and successor.get('schema')=='owned_fixture_setup_successor_v1'
                  and successor.get('owner_sha256')==r.digest(owner)
                  and successor.get('release_ref')==owner['release_ref']
                  and successor.get('executor_identity')==executor
                  and successor.get('prior_executor')==owner['coordinator_identity']
                  and successor.get('host_binding')==self.binding
                  and successor.get('prior_exact_executor_absent') is True
                  and successor.get('attempt_replay_permitted') is False
                  and self.io.process(owner['coordinator_identity']['pid']) is None,
                  'invalid setup successor authority')
        for role in ROLES:r.require(successor['plist_refs'][role]==r.reference(self.root/(role+'.plist')),'successor plist changed')

    def remember_ports(self,role):
        if self.spec:self.known_ports[role].add(self.spec['anchors'][role]['sentinel_port'])
        for directory in (self.root/'setup-host',self.root,self.root/'cleanup'):
            for path in sorted(directory.glob('host-operation-*.json')):
                row=r.read(path)
                if row.get('operation')=='bootstrap_ready' and row.get('role')==role:
                    self.known_ports[role].add(row['sentinel_port'])
        path=self.root/(('provider_ready' if role=='provider' else 'startup')+'-events.json')
        if path.exists():self.known_ports[role].add(r.read(path)['sentinel_port'])
        if (self.root/'setup/header.json').exists():
            ledger=r.Ledger(self.root,'setup',{'owner_sha256':r.digest(self.owner),'release_sha256':self.owner['release_ref']['sha256']})
            result=ledger.completed().get('bootstrap:'+role)
            if result:self.known_ports[role].add(result['sentinel_port'])
        for port in self.known_ports[role]:r.require(type(port) is int and 1024<=port<=65535,'invalid retained private port')

    def cleanup_held(self,role,context):
        from owned_fixture_cleanup_host import held
        return held(self,role,context)
    def cleanup_final(self,context,require_guards_absent=False):
        from owned_fixture_cleanup_host import final
        return final(self,context,require_guards_absent=require_guards_absent)
    def disable_cleanup(self):
        self.cleanup=None;self.cleanup_authorizer=None
