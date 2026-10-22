"""Synthetic transactions only. No service requests or real commands."""
import ast, copy, hashlib, json, os
from pathlib import Path
import tempfile, unittest
from unittest.mock import patch
import production_recovery_inspect as r
import production_native as native

class FakeIO:
    def __init__(self,plan):
        self.plan=plan
        self.roles={role:{'target':f"gui/{os.getuid()}/{r.m.LABELS[role]}",'job_loaded':True,
            'process':copy.deepcopy(v['identity']),'listener_pids':[v['identity']['pid']],'held':None} for role,v in plan['roles'].items()}
        self.processes={v['pid']:v for v in plan['protected'].values()}
        self.processes.update({v['identity']['pid']:v['identity'] for v in plan['roles'].values()})
    def host(self):return copy.deepcopy(self.plan['host_binding'])
    def stewardship(self):return {'paused':True,'pause_generation':1,'pause_actor':'test','lease':None}
    def role(self,role):return copy.deepcopy(self.roles[role])
    def process(self,pid):return copy.deepcopy(self.processes.get(pid))
    def all_listeners(self,pid):return []

class RecoveryTests(unittest.TestCase):
    def setUp(self):
        # These legacy history fixtures predate command-child evidence. The new
        # native proof boundary has its own inert tests; keep this suite focused
        # on original controller/Host reconciliation semantics.
        for name,value in (('retained_commands',[]),('host_links',None)):
            mock=patch.object(native,name,return_value=value);mock.start();self.addCleanup(mock.stop)
        temp=tempfile.TemporaryDirectory(prefix='recovery-fake-');self.addCleanup(temp.cleanup)
        self.base=Path(temp.name).resolve();self.tx=self.base/'transactions'/('a'*32);self.tx.mkdir(parents=True,mode=0o700)
        self.journal=self.tx/'controller';self.journal.mkdir(mode=0o700)
        workspace=self.base/'workspace';workspace.mkdir()
        sources={role:workspace/(role+'.py') for role in r.m.ROLES};plists={role:workspace/(role+'.plist') for role in r.m.ROLES}
        holder=self.make('holder-python','fake qualified 3.14');python=self.make('original-python','fake original 3.12')
        launcher=self.make('model_environment.py','# fake holder source')
        for name,value in {'CONTROL_ROOT':self.tx.parent,'WORKSPACE':workspace,'STATE_ROOT':workspace/'state','HOLD_SOURCE':launcher,
            'SOURCES':sources,'PLISTS':plists,'LOGS':{role:workspace/(role+'.log') for role in r.m.ROLES},'QOS':workspace/'qos.jsonl'}.items():
            p=patch.object(r.p,name,value);p.start();self.addCleanup(p.stop)
        p=patch.object(r,'HOLDER_PYTHON',str(holder));p.start();self.addCleanup(p.stop)
        p=patch.object(r.p.subprocess,'run',side_effect=AssertionError('real command forbidden'));self.commands=p.start();self.addCleanup(p.stop)
        files={str(p):self.sha(p) for p in (holder,python,launcher)};roles={};originals={}
        for n,role in enumerate(r.m.ROLES):
            sources[role].write_text('# original '+role);plists[role].write_text('original settings '+role);originals[role]={}
            for kind,path in (('source',sources[role]),('plist',plists[role])):
                saved=self.make(role+'-original-'+kind,path.read_text());originals[role][kind]=str(saved)
                files[str(saved)]=files[str(path)]=self.sha(path)
            roles[role]={'label':r.m.LABELS[role],'port':r.m.PORTS[role],'identity':{'pid':100+n,'started_at':'old-'+role,'argv':[str(python),str(sources[role])]}}
        candidate=self.make('candidate.py','# scheduling repair');pinned=self.make('pinned.plist','pinned snapshot settings')
        inputs={key:'b'*64 for key in r.m.REQUIRED_INPUTS}
        for role in r.m.ROLES:inputs.update({role+'_source':self.sha(sources[role]),role+'_settings':self.sha(plists[role])})
        inputs.update(reservoir_candidate=self.sha(candidate),provider_restart_settings=self.sha(pinned))
        self.plan=r.m.validate_plan({'schema':'reservoir_maintenance_plan_v1','transaction_id':'a'*32,'actor':'test','pause_generation':1,
            'host_binding':{'uid':os.getuid(),'boot_uuid':'fake-boot','hostname':'fake','os_version':'fake'},'input_hashes':inputs,'roles':roles,
            'protected':{name:{'pid':200+n,'started_at':'protected','argv':['fake',name]} for n,name in enumerate(sorted(r.p.PROTECTED))}})
        self.spec={'schema':'reservoir_production_preparation_v1','plan':self.plan,'transaction_root':str(self.tx),'state_root':str(r.p.STATE_ROOT),
            'immutable_files':files,'originals':originals,'repair_source':self.ref(candidate),'provider_restart_plist':self.ref(pinned),
            'provider_restart_argv':roles['provider']['identity']['argv']+['--model','fake-snapshot'],
            'input_groups':{'model_snapshot':[],'interpreter':[str(python)],'helpers':[str(launcher)],'provider_packages':[]}}
        for key in ('fixture','runtime_alignment','model_resolution','original_provider_manifest'):
            data={'unit_test_only':True}
            if key=='fixture':data['python_executables']={str(holder):self.sha(holder)}
            path=self.base/(key+'.json');self.write(path,data);self.spec[key]=self.ref(path)
        (self.tx/'model_environment.py').write_bytes(launcher.read_bytes());self.prep=self.base/'preparation.json';self.save_spec()
        for role in r.m.ROLES:
            anchor={'identity':roles[role]['identity'],'log':self.cursor(r.p.LOGS[role])}
            if role=='provider':anchor.update(qos=self.cursor(r.p.QOS),start_worker_idle=True,start_queue_empty=True)
            else:anchor.update(inventory={'type':'list_handles_response','handles':[{'name':'test'}]},inventory_matches_start_boundary=True)
            self.write(self.tx/(role+'-capture-anchor.json'),anchor)
        self.write(self.journal/'plan.json',self.plan);self.events=[];self.hosts=[];self.io=FakeIO(self.plan)
    def make(self,name,text):
        p=self.base/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text);return p
    def sha(self,path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
    def ref(self,path):return {'path':str(path),'sha256':self.sha(path)}
    def write(self,path,value):path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value))
    def save_spec(self):
        self.write(self.prep,self.spec);self.write(self.tx/'owner.json',{'schema':'reservoir_production_owner_v1','transaction_id':'a'*32,
            'actor':'test','host_binding':self.plan['host_binding'],'preparation_sha256':r.p.preparation_digest(self.spec)})
    def cursor(self,path):return {'path':str(path),'device':1,'inode':2,'start_offset':0,'prefix_sha256':'a'*64}
    def guard(self):return {'schema':'model_environment_hold_v1','transaction_id':'a'*32,'actor':'test'}
    def event(self,event_type,**kw):
        row={'sequence':len(self.events)+1,'previous_sha256':r.m.digest(self.events[-1]) if self.events else r.m.digest(self.plan),
             'plan_sha256':r.m.digest(self.plan),'type':event_type,**kw};self.events.append(row)
        self.write(self.journal/f'event-{len(self.events):05d}.json',row)
    def host(self,operation,**kw):
        row={'operation':operation,'monotonic':float(len(self.hosts)+1),**kw};self.hosts.append(row)
        self.write(self.tx/f'host-operation-{len(self.hosts):05d}.json',row)
    def intent(self,op,role):self.event('mutation_intent',operation=op,role=role)
    def done(self,op,role,result):self.event('mutation_completed',operation=op,role=role,result=result)
    def report(self):
        result=r.inspect(self.prep,self.journal,io=self.io)
        for key in ('automatic_resume_allowed','retry_authorized','production_recovery_qualified','mutation_performed'):self.assertFalse(result[key])
        self.commands.assert_not_called();return result
    def hold(self,role='provider'):
        self.intent('create_hold',role);self.host('hold_intent',role=role,guard=self.guard())
        self.write(self.tx/('hold-'+role)/'hold.json',self.guard());self.host('hold_created',role=role,guard=self.guard());self.done('create_hold',role,{'guard':self.guard()})
    def debug(self,result=True,complete=True):
        self.hold();self.intent('arm_next_spawn','provider')
        argv=['/usr/bin/sudo','--','/bin/launchctl','debug',f'gui/{os.getuid()}/{r.m.LABELS["provider"]}','--program',r.HOLDER_PYTHON,'--',str(self.tx/'model_environment.py'),
              'launch','--control-root',str(self.tx/'hold-provider'),'--hold-only','a'*32]
        self.host('debug_intent',role='provider',command=argv,identity=self.plan['roles']['provider']['identity'])
        if result:self.host('debug_result',role='provider',returncode=0)
        if complete:self.done('arm_next_spawn','provider',{'armed':True})
    def signal(self,result=True,complete=True,boundary=True):
        self.debug();self.intent('signal_stop','provider')
        self.host('signal_intent',role='provider',identity=self.plan['roles']['provider']['identity'],signal='SIGTERM',**({'wall_time_ns':123456789} if boundary else {}))
        if result:self.host('signal_sent',role='provider',identity=self.plan['roles']['provider']['identity'])
        if complete:self.done('signal_stop','provider',{'signal':'SIGTERM','identity':self.plan['roles']['provider']['identity'],'force_used':False})
    def holder(self,receipt=True,wrong_family=False):
        argv=[self.spec['input_groups']['interpreter'][0] if wrong_family else r.HOLDER_PYTHON,str(self.tx/'model_environment.py'),'launch','--control-root',str(self.tx/'hold-provider'),'--hold-only','a'*32]
        ident={'pid':500,'started_at':'holder-start','argv':argv};self.io.roles['provider'].update(process=ident,listener_pids=[]);self.io.processes.pop(100,None)
        if receipt:self.write(self.tx/'hold-provider'/'held'/'500.json',{'schema':'model_environment_held_v1','pid':500,'started_at':'holder-start','hold':self.guard(),
            'hold_only':True,'model_imported':False,'listener_opened':False,'launcher_sha256':self.sha(self.tx/'model_environment.py')})
        return ident
    def valid(self,result):self.assertTrue(result['evidence_valid'],result['errors'])

    def test_empty_transaction_and_execution_review_exclusion(self):
        self.spec['execution_review']={'path':'/not/read','sha256':'f'*64};self.write(self.prep,self.spec)
        result=self.report();self.valid(result);self.assertFalse(result['unresolved'])
    def test_debug_without_result_stays_unresolved_even_with_owned_holder(self):
        self.debug(result=False,complete=False);self.holder();result=self.report();self.valid(result)
        self.assertEqual(result['roles']['provider']['classification'],'owned_holder_observed');self.assertTrue(result['unresolved'])
        self.assertEqual(result['possibly_pending_debug_roles'],['provider'])
    def test_debug_result_before_controller_completion_is_unresolved(self):
        self.debug(complete=False);result=self.report();self.valid(result);self.assertTrue(result['unresolved'])
    def test_signal_before_and_after_result_never_authorizes_retry(self):
        self.signal(result=False,complete=False);result=self.report();self.valid(result);self.assertTrue(result['unresolved'])
        self.assertEqual(result['signal_wall_time_ns'],{'provider':123456789})
        self.host('signal_sent',role='provider',identity=self.plan['roles']['provider']['identity']);self.done('signal_stop','provider',{'signal':'SIGTERM','identity':self.plan['roles']['provider']['identity'],'force_used':False})
        self.valid(self.report())
    def test_missing_signal_wall_time_is_not_invented(self):
        self.signal(boundary=False);result=self.report();self.valid(result)
        self.assertEqual(result['signal_wall_time_ns'],{});self.assertTrue(any('boundary unavailable' in x for x in result['unresolved']))
    def test_held_removal_intent_keeps_debug_uncertain_until_results(self):
        self.signal();held=self.holder();self.event('held_verified',role='provider',identity=held)
        self.intent('remove_held','provider');self.host('held_bootout_intent',role='provider',identity=held)
        self.io.roles['provider'].update(job_loaded=False,process=None,listener_pids=[])
        result=self.report();self.valid(result);self.assertEqual(result['possibly_pending_debug_roles'],['provider']);self.assertTrue(result['unresolved'])
        self.host('held_job_absent',role='provider',identity=held);self.done('remove_held','provider',{'removed_identity':held})
        result=self.report();self.valid(result);self.assertEqual(result['possibly_pending_debug_roles'],[])
    def test_install_bytes_never_resolve_missing_operation_result(self):
        self.intent('install_repair',None);self.host('install_intent',target=str(r.p.SOURCES['reservoir']),source_sha256=self.spec['repair_source']['sha256'])
        r.p.SOURCES['reservoir'].write_bytes(Path(self.spec['repair_source']['path']).read_bytes())
        result=self.report();self.valid(result);self.assertEqual(result['installed_variants']['reservoir']['source'],'candidate');self.assertTrue(result['unresolved'])
        self.host('install_completed',source_sha256=self.spec['repair_source']['sha256']);self.done('install_repair',None,{'source_sha256':self.spec['repair_source']['sha256']});self.valid(self.report())
    def test_plist_and_bootstrap_before_after_records_and_pending_pid(self):
        self.intent('apply_provider_restart','provider');self.host('provider_snapshot_plist_intent',original_sha256=self.plan['input_hashes']['provider_settings'],restart_sha256=self.spec['provider_restart_plist']['sha256'])
        r.p.PLISTS['provider'].write_bytes(Path(self.spec['provider_restart_plist']['path']).read_bytes())
        result=self.report();self.valid(result);self.assertTrue(result['unresolved'])
        self.host('provider_snapshot_plist_installed',restart_sha256=self.spec['provider_restart_plist']['sha256'])
        self.done('apply_provider_restart','provider',{'provider_settings_sha256':self.spec['provider_restart_plist']['sha256'],
                  'provider_restart_argv':self.spec['provider_restart_argv']})
        self.intent('bootstrap_original','provider')
        self.host('bootstrap_intent',role='provider',applied_plist_sha256=self.spec['provider_restart_plist']['sha256'],original_rollback_plist_sha256=self.plan['input_hashes']['provider_settings'],log_anchor=self.cursor(r.p.LOGS['provider']))
        self.io.roles['provider'].update(process=None,listener_pids=[]);result=self.report();self.valid(result)
        self.assertEqual(result['roles']['provider']['classification'],'pending_or_inconsistent_no_process')
        new={'pid':600,'started_at':'new','argv':self.spec['provider_restart_argv']};self.host('bootstrap_ready',role='provider',identity=new);self.done('bootstrap_original','provider',{'identity':new})
        self.io.roles['provider'].update(process=new,listener_pids=[600]);self.io.processes.pop(100)
        result=self.report();self.valid(result);self.assertEqual(result['roles']['provider']['classification'],'recorded_replacement_observed')
    def test_plist_completion_requires_separate_action_and_exact_restart_argv(self):
        self.intent('apply_provider_restart','provider')
        self.host('provider_snapshot_plist_intent',original_sha256=self.plan['input_hashes']['provider_settings'],restart_sha256=self.spec['provider_restart_plist']['sha256'])
        self.host('provider_snapshot_plist_installed',restart_sha256=self.spec['provider_restart_plist']['sha256'])
        self.done('apply_provider_restart','provider',{'provider_settings_sha256':self.spec['provider_restart_plist']['sha256'],
                  'provider_restart_argv':['foreign']})
        self.assertFalse(self.report()['evidence_valid'])

    def restore_launch(self):
        here=self.base/'fake-harness';here.mkdir()
        for name in ('restore_verifier.py','reservoir_maintenance_state.py'):
            path=here/name;path.write_text('# fake '+name);self.spec['immutable_files'][str(path)]=self.sha(path)
        change=patch.object(r.p,'HERE',here);change.start();self.addCleanup(change.stop);self.save_spec()
        backup={'schema':'reservoir_maintenance_witness_v1','kind':'backup','passed':True,'transaction_id':self.plan['transaction_id'],
                'evidence':{'synthetic_only':True},'bindings':{'plan_sha256':r.m.digest(self.plan),
                **{role+'_old_identity_sha256':r.m.digest(self.plan['roles'][role]['identity']) for role in r.m.ROLES}}}
        self.event('witness',kind='backup',receipt=backup);self.write(self.tx/'backup-witness.json',backup)
        request_path=self.tx/'private-restore-request.json'
        self.write(request_path,{'schema':'reservoir_private_restore_request_v1','passed':False,
                  'context':{'plan':self.plan,'transaction_id':self.plan['transaction_id']},'backup_witness':backup})
        binding={'transaction_id':self.plan['transaction_id'],'plan_sha256':r.m.digest(self.plan),'preparation_sha256':r.p.preparation_digest(self.spec),
                 'request':self.ref(request_path),'backup_sha256':r.m.digest(backup),'work_root':str(here/'restore-runs'/('production-'+self.plan['transaction_id'])),
                 'argv':[str(r.p.WORKSPACE/'.venv/bin/python'),'-B',str(here/'restore_verifier.py'),'--request',str(request_path),'--execute-private-restore'],
                 'implementation':{str(path):self.sha(path) for path in (here/'restore_verifier.py',here/'reservoir_maintenance_state.py',r.p.HOLD_SOURCE)}}
        self.write(self.tx/'private-restore-launch.json',{'schema':'reservoir_production_restore_launch_v1',**binding,
                   'started_wall':100.,'timeout_s':300,'automatic_retry_permitted':False})
        return binding

    def restore_start_exit(self,binding,code=0):
        self.write(self.tx/'private-restore-child-start.json',{'schema':'reservoir_production_restore_child_start_v1',**binding,
                   'pid':999,'started_wall':101.,'signals_permitted':False})
        refs={}
        for name in ('stdout','stderr'):
            path=self.tx/('private-restore-'+name+'.txt');path.write_text('fake output');refs[name]=self.ref(path)
        self.write(self.tx/'private-restore-child-exit.json',{'schema':'reservoir_production_restore_child_exit_v1',**binding,
                   'pid':999,'returncode':code,'completed_wall':102.,'reaped':True,**refs})

    def test_restore_launch_and_child_start_always_block_until_exit_and_outer_witness(self):
        binding=self.restore_launch();result=self.report();self.valid(result)
        self.assertEqual(result['private_restore_execution']['status'],'unresolved')
        self.write(self.tx/'private-restore-child-start.json',{'schema':'reservoir_production_restore_child_start_v1',**binding,
                   'pid':999,'started_wall':101.,'signals_permitted':False})
        result=self.report();self.valid(result);self.assertTrue(result['unresolved'])
        self.restore_start_exit(binding);result=self.report();self.valid(result)
        self.assertEqual(result['private_restore_execution']['status'],'exited_without_outer_witness')
        self.assertTrue(any('outer accepted witness' in item for item in result['unresolved']))

    def test_restore_nonzero_or_unresolved_marker_never_implies_reaped_success(self):
        binding=self.restore_launch();self.restore_start_exit(binding,code=1)
        result=self.report();self.valid(result);self.assertEqual(result['private_restore_execution']['status'],'unresolved')
        self.restore_start_exit(binding)
        self.write(self.tx/'private-restore-unresolved.json',{'schema':'reservoir_production_restore_unresolved_v1',**binding,
                   'pid':999,'error':'synthetic failure','may_still_be_running':False,'automatic_retry_permitted':False,
                   'next_child_permitted':False,'force_used':False,'production_install_permitted':False})
        result=self.report();self.valid(result);self.assertEqual(result['private_restore_execution']['status'],'unresolved')

    def test_restore_request_only_or_child_without_launch_not_retryable(self):
        self.write(self.tx/'private-restore-request.json',{'synthetic_only':True})
        result=self.report();self.valid(result);self.assertEqual(result['private_restore_execution']['status'],'request_only')
        self.write(self.tx/'private-restore-child-start.json',{'schema':'reservoir_production_restore_child_start_v1','pid':999})
        self.assertFalse(self.report()['evidence_valid'])

    def test_restore_binding_pid_and_log_mutations_refuse(self):
        binding=self.restore_launch();self.restore_start_exit(binding)
        path=self.tx/'private-restore-child-exit.json';original=path.read_bytes()
        for key,value in (('pid',1000),('returncode',None),('reaped',False),('preparation_sha256','f'*64)):
            row=json.loads(original);row[key]=value;self.write(path,row)
            self.assertFalse(self.report()['evidence_valid']);path.write_bytes(original)
        (self.tx/'private-restore-stdout.txt').write_text('changed');self.assertFalse(self.report()['evidence_valid'])

    def test_outer_restore_witness_requires_recorded_clean_exit(self):
        binding=self.restore_launch();self.restore_start_exit(binding)
        receipt={'schema':'reservoir_maintenance_witness_v1','kind':'private_restore','passed':True,'transaction_id':self.plan['transaction_id'],
                 'evidence':{'synthetic_only':True},'bindings':{'plan_sha256':r.m.digest(self.plan),
                 **{role+'_old_identity_sha256':r.m.digest(self.plan['roles'][role]['identity']) for role in r.m.ROLES}}}
        self.event('witness',kind='private_restore',receipt=receipt);self.write(self.tx/'private_restore-witness.json',receipt)
        result=self.report();self.valid(result);self.assertEqual(result['private_restore_execution']['status'],'outer_witness_recorded')
        (self.tx/'private-restore-child-exit.json').unlink();self.assertFalse(self.report()['evidence_valid'])
    def test_wrong_holder_interpreter_family_is_unknown(self):
        self.debug();self.holder(wrong_family=True);result=self.report();self.valid(result)
        self.assertEqual(result['roles']['provider']['classification'],'unknown_process')
    def test_missing_holder_receipt_or_foreign_guard_cannot_verify(self):
        self.debug();self.holder(receipt=False);result=self.report();self.valid(result)
        self.assertEqual(result['roles']['provider']['classification'],'holder_receipt_pending_or_missing')
        self.write(self.tx/'hold-provider'/'hold.json',dict(self.guard(),actor='foreign'));self.assertFalse(self.report()['evidence_valid'])
    def test_missing_or_altered_witness_refuses(self):
        value={'schema':'reservoir_maintenance_witness_v1','kind':'provider_drain','passed':True,'transaction_id':'a'*32,'evidence':{'unit_test_only':True},
            'bindings':{'plan_sha256':r.m.digest(self.plan),**{role+'_old_identity_sha256':r.m.digest(self.plan['roles'][role]['identity']) for role in r.m.ROLES}}}
        self.event('witness',kind='provider_drain',receipt=value);self.assertFalse(self.report()['evidence_valid'])
        self.write(self.tx/'provider_drain-witness.json',dict(value,passed=False));self.assertFalse(self.report()['evidence_valid'])
        self.write(self.tx/'provider_drain-witness.json',value);self.valid(self.report())
    def test_chain_owner_anchor_and_host_sequence_corruption_refuse(self):
        self.debug()
        for path,change in [(self.journal/'event-00001.json',lambda x:x.update(previous_sha256='f'*64)),
             (self.tx/'owner.json',lambda x:x.update(actor='foreign')),
             (self.tx/'provider-capture-anchor.json',lambda x:x['identity'].update(pid=999))]:
            original=path.read_bytes();value=json.loads(original);change(value);self.write(path,value)
            self.assertFalse(self.report()['evidence_valid']);path.write_bytes(original)
        (self.tx/'host-operation-00002.json').unlink();self.assertFalse(self.report()['evidence_valid'])
    def test_unknown_disk_bytes_are_reported_without_loaded_source_inference(self):
        r.p.SOURCES['reservoir'].write_text('# unrelated');result=self.report();self.valid(result)
        self.assertEqual(result['installed_variants']['reservoir']['source'],'unknown');self.assertTrue(result['unresolved'])
    def test_bound_model_alias_allowed_but_record_symlink_refuses(self):
        target=self.make('blob','fake model bytes');alias=self.base/'snapshot';alias.symlink_to(target)
        self.spec['immutable_files'][str(alias)]=self.sha(target);self.spec['input_groups']['model_snapshot']=[str(alias)];self.save_spec()
        result=self.report();self.valid(result);self.assertEqual(result['artifacts'][str(alias)]['immutable_alias']['target'],str(target))
        saved=self.base/'saved-owner';owner=self.tx/'owner.json';owner.rename(saved);owner.symlink_to(saved);self.assertFalse(self.report()['evidence_valid'])
    def test_alias_change_and_absence_change_are_detected(self):
        target=self.make('blob','fake');alias=self.base/'alias';alias.symlink_to(target);e=r.Evidence();e.read(alias,allow_alias=True)
        alias.unlink();alias.symlink_to(target)
        with self.assertRaises(r.m.Refusal):e.finish()
        e=r.Evidence();e.absent(self.base/'missing');self.make('missing','new')
        with self.assertRaises(r.m.Refusal):e.finish()
    def test_new_record_during_observation_is_inconclusive(self):
        before=self.io.host;calls=0
        def host():
            nonlocal calls
            calls+=1
            if calls==2:self.host('hold_intent',role='provider',guard=self.guard())
            return before()
        self.io.host=host
        result=self.report();self.assertFalse(result['evidence_valid'])
        self.assertTrue(any('record set changed' in x for x in result['errors']))

    def test_bootstrap_command_return_is_recorded_separately_from_readiness(self):
        self.intent('bootstrap_original','provider')
        self.host('bootstrap_intent',role='provider',wall_time_ns=1000,
                  applied_plist_sha256=self.plan['input_hashes']['provider_restart_settings'],
                  original_rollback_plist_sha256=self.plan['input_hashes']['provider_settings'],log_anchor=self.cursor(r.p.LOGS['provider']))
        self.host('bootstrap_command_returned',role='provider',returncode=0,wall_time_ns=2000)
        new={'pid':600,'started_at':'new','argv':self.spec['provider_restart_argv']}
        self.io.roles['provider'].update(process=new,listener_pids=[600]);self.io.processes.pop(100)
        report=self.report();self.valid(report)
        self.assertEqual(report['bootstrap_wall_time_ns'],{'provider':1000})
        self.assertEqual(report['roles']['provider']['classification'],'unrecorded_replacement_candidate')
        self.assertTrue(report['unresolved'])
        self.host('bootstrap_ready',role='provider',identity=new)
        report=self.report();self.valid(report)
        path=self.tx/'host-operation-00002.json';row=json.loads(path.read_text());row['returncode']=1;self.write(path,row)
        self.assertFalse(self.report()['evidence_valid'])

    def test_partial_outputs_are_reconciliation_only_without_replaying_producer(self):
        report=self.report();self.valid(report);self.assertFalse(any(report['recovery_output_presence'].values()))
        (self.tx/'provider-drain.log').write_text('partial capture')
        (self.tx/'stopped-backup').mkdir(mode=0o700)
        report=self.report();self.valid(report)
        self.assertTrue(report['recovery_output_presence']['provider_drain'])
        self.assertTrue(report['recovery_output_presence']['backup'])
        self.assertFalse(report['recovery_output_presence']['startup'])

    def test_recovery_action_result_must_match_its_actual_host_result_span(self):
        self.debug(complete=False)
        # Remove the original debug controller intent from this direct parser
        # fixture to exercise recovery authority instead of fabricated history.
        evidence=r.Evidence();control=r.controller_records(self.journal,self.plan,evidence)
        control['intents'].pop(('arm_next_spawn','provider'))
        rows=[{'type':'action_claim','action_id':'arm_next_spawn:provider','operation':'arm_next_spawn','role':'provider'},
              {'type':'action_result','action_id':'arm_next_spawn:provider','operation':'arm_next_spawn','role':'provider',
               'result':{'armed':True},'host_operations':{'before_count':2,'after_count':4}}]
        parsed=r.host_records(self.tx,self.plan,control,evidence,rows)
        self.assertEqual(parsed['pairs'][('debug_intent','provider')]['result']['returncode'],0)
        path=self.tx/'host-operation-00004.json';row=json.loads(path.read_text());row['returncode']=1;self.write(path,row)
        with self.assertRaisesRegex(r.m.Refusal,'result mismatch'):
            r.host_records(self.tx,self.plan,control,r.Evidence(),rows)

    def test_read_only_io_rejects_mutations_and_service_requests(self):
        io=r.ReadOnlyIO()
        for argv in (('/bin/launchctl','bootstrap','gui/501','x'),('/usr/bin/sudo','--','true'),('/bin/kill','123')):
            with self.assertRaises(r.m.Refusal):io.command(*argv)
        for method in (io.health,io.list_handles):
            with self.assertRaises(r.m.Refusal):method()
        self.commands.assert_not_called()
    def test_cli_without_args_and_ast_cannot_mutate(self):
        with patch('sys.argv',['inspector']),patch.object(r,'inspect') as inspect:
            with self.assertRaises(SystemExit):r.main()
            inspect.assert_not_called()
        forbidden={'kill','terminate','send_signal','bootstrap_original','arm_next_spawn','signal_stop','release_hold','write_text','write_bytes','write_new','mkdir','unlink','replace'}
        calls=[n.func.attr for n in ast.walk(ast.parse(Path(r.__file__).read_text())) if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr in forbidden]
        self.assertEqual(calls,[])

if __name__=='__main__':unittest.main(verbosity=2)
