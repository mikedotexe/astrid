"""Retained-file boundary tests. No service, process, network or numerical APIs."""
import ast
import copy
import hashlib
import json
import os
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

import production_fixture_admission as a
import production_fixture_history as h


def reference(path):
    return {'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}


def write(path, value):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(value,sort_keys=True)+'\n')
    return reference(path)


def ident(pid=11):
    return {'pid':pid,'started_at':'Fri Oct 16 12:00:00 2026','argv':['/private/fake-python','inert.py']}


def chain(header, entries):
    rows=[]; previous=a.digest(header)
    for value in entries:
        row={'sequence':len(rows)+1,'previous':previous,'wall_time_ns':len(rows)+1,**value}
        rows.append(row); previous=a.digest(row)
    return rows


class Temporary(unittest.TestCase):
    def setUp(self):
        self.temporary=tempfile.TemporaryDirectory(); self.addCleanup(self.temporary.cleanup)
        self.root=Path(self.temporary.name).resolve()


class StableFiles(Temporary):
    def test_bound_object_and_stream_hash(self):
        ref=write(self.root/'one.json',{'one':1}); files=a.Files()
        self.assertEqual(files.ref(ref),{'one':1})
        self.assertEqual(files.ref(ref,object=False),ref['sha256']); files.finish()

    def test_modified_reference_and_after_read_change_refused(self):
        path=self.root/'one.json'; ref=write(path,{'one':1}); files=a.Files(); files.ref(ref)
        write(path,{'two':2})
        with self.assertRaisesRegex(ValueError,'changed'): files.finish()
        with self.assertRaisesRegex(ValueError,'hash differs'): a.Files().ref(ref)

    def test_duplicate_nonfinite_and_nonobject_json_refused(self):
        for raw in (b'{"x":1,"x":2}',b'{"x":NaN}',b'[]'):
            with self.subTest(raw=raw):
                path=self.root/'bad.json'; path.write_bytes(raw)
                with self.assertRaises(ValueError): a.Files().json(path)

    def test_symlink_hardlink_and_size_refused(self):
        path=self.root/'one.json'; write(path,{'one':1})
        link=self.root/'alias'; link.symlink_to(path)
        with self.assertRaisesRegex(ValueError,'noncanonical'): a.Files().read(link)
        hard=self.root/'hard'; os.link(path,hard)
        with self.assertRaisesRegex(ValueError,'unsafe'): a.Files().read(path)
        hard.unlink()
        with patch.object(a,'MAX_JSON',3):
            with self.assertRaisesRegex(ValueError,'oversized'): a.Files().json(path)

    def test_closed_reference_set_and_extra_history(self):
        ref=write(self.root/'one.json',{'one':1}); files=a.Files(); files.allowed=set()
        with self.assertRaisesRegex(ValueError,'outside complete'): files.ref(ref)
        expected={ref['path']:ref['sha256']}; a.inventory(a.Files(),self.root,expected,[])
        write(self.root/'unlisted.json',{'extra':True})
        with self.assertRaisesRegex(ValueError,'unlisted'): a.inventory(a.Files(),self.root,expected,[])

    def test_only_empty_owned_lock_may_be_excluded(self):
        lock=self.root/'execution.lock'; lock.write_bytes(b''); lock.chmod(0o600)
        ref=write(self.root/'one.json',{}); expected={ref['path']:ref['sha256']}
        a.inventory(a.Files(),self.root,expected,[str(lock)])
        lock.write_bytes(b'unlisted payload')
        with self.assertRaisesRegex(ValueError,'unsafe excluded'): a.inventory(a.Files(),self.root,expected,[str(lock)])

    def test_run_size_file_and_directory_bounds_precede_content_reads(self):
        ref=write(self.root/'one.json',{'one':1}); expected={ref['path']:ref['sha256']}
        for name,value in (('MAX_RUN_BYTES',1),('MAX_RUN_FILES',0),('MAX_RUN_DIRECTORIES',0)):
            with self.subTest(limit=name), patch.object(a,name,value):
                with self.assertRaises(ValueError): a.inventory(a.Files(),self.root,expected,[])
        files=a.Files(); files.bind_run(self.root)
        with patch.object(a,'MAX_RUN_BYTES',1):
            with self.assertRaisesRegex(ValueError,'oversized|budget'): files.read(ref['path'])

    def test_private_directories_and_exact_lock_exclusions_required(self):
        ref=write(self.root/'one.json',{}); expected={ref['path']:ref['sha256']}
        directory=self.root/'nested'; directory.mkdir(mode=0o755)
        with self.assertRaisesRegex(ValueError,'public'): a.inventory(a.Files(),self.root,expected,[])
        directory.chmod(0o700)
        with patch.object(a.os,'getuid',return_value=os.getuid()+1):
            with self.assertRaisesRegex(ValueError,'unowned'): a.inventory(a.Files(),self.root,expected,[])
        directory.rmdir(); directory.symlink_to(self.root,target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'noncanonical'): a.inventory(a.Files(),self.root,expected,[])
        directory.unlink(); lock=self.root/'execution.lock'; lock.write_bytes(b''); lock.chmod(0o600)
        for exclusions in ([],[str(lock),str(lock)],[str(self.root/'foreign.lock')]):
            with self.subTest(exclusions=exclusions):
                with self.assertRaisesRegex(ValueError,'exclusion'): a.inventory(a.Files(),self.root,expected,exclusions)

    def test_memoization_is_per_invocation_and_rechecks_mode(self):
        path=self.root/'one.json'; ref=write(path,{'one':1}); path.chmod(0o640); first=a.Files(); first.ref(ref)
        path.chmod(0o600)
        with self.assertRaisesRegex(ValueError,'changed'): first.ref(ref)
        second=a.Files(); self.assertEqual(second.ref(ref),{'one':1}); second.finish()


class FiniteSchedule(unittest.TestCase):
    def test_seventy_one_steps_and_exact_interruption_successors(self):
        steps,manifest=a.schedule()
        self.assertEqual(len(steps),71); self.assertEqual(len({r['id'] for r in steps}),71)
        self.assertEqual([r['kind'] for r in steps[:4]],['lock_hold','lock_refusal','fresh','resume_one'])
        self.assertEqual([r['expected_exit'] for r in steps[:4]],[0,76,75,0])
        self.assertEqual(len(manifest['points']),23)
        for index,row in enumerate(steps):
            self.assertNotIn('apply_provider_restart',row['action_id'])
            if row['injection']:
                after=steps[index+1]
                self.assertEqual(after['mode'],'reconcile'); self.assertEqual(after['action_id'],row['action_id'])
                self.assertEqual(after['expected_exit'],0)
        self.assertEqual(steps[-1]['action_id'],'complete'); self.assertEqual(steps[-1]['kind'],'pin')

    def test_chain_preserves_uncertain_claim_before_adoption(self):
        header={'schema':'owned_fixture_ledger_v1'}
        rows=chain(header,[{'type':'claim','action':'one','executor_identity':ident()}])
        rows=chain(header,[rows[0],{'type':'reconcile_claim','action':'one','executor_identity':ident(12)}])
        claims=[a.digest(r) for r in rows]
        rows=chain(header,[*rows,{'type':'reconciled','action':'one','claim_sha256':claims[-1],
                                'resolves_claims':claims,'proof':{'passed':True},'result':{'observed':True}}])
        result,_=a.fixture_chain(header,rows,['one']); self.assertEqual(result,{'one':{'observed':True}})
        changed=copy.deepcopy(rows); changed[-1]['resolves_claims']=claims[-1:]
        with self.assertRaisesRegex(ValueError,'all claims'): a.fixture_chain(header,changed,['one'])

    def test_replay_missing_result_gap_and_wrong_action_refused(self):
        header={}; claim={'type':'claim','action':'one','executor_identity':ident()}
        for entries in ([claim],[claim,claim],[{**claim,'action':'foreign'}]):
            with self.subTest(entries=entries):
                with self.assertRaises(ValueError): a.fixture_chain(header,chain(header,entries),['one'])
        rows=chain(header,[claim]); rows[0]['sequence']=2
        with self.assertRaisesRegex(ValueError,'chain differs'): a.fixture_chain(header,rows,['one'])

    def test_failed_action_cannot_be_relabelled_ordinary_success(self):
        header={}; rows=chain(header,[{'type':'claim','action':'one','executor_identity':ident()}]); claim=a.digest(rows[0])
        rows=chain(header,[*rows,{'type':'failed','action':'one','claim_sha256':claim},
                          {'type':'result','action':'one','claim_sha256':claim,'result':{}}])
        with self.assertRaisesRegex(ValueError,'uncertain'): a.fixture_chain(header,rows,['one'])


class InterpreterAndOrigin(Temporary):
    def test_reader_worker_dependency_matches_producer_without_importing_host(self):
        producer=ast.parse((a.FIXTURE/'owned_recovery_host.py').read_text())
        assignments=[node.value for node in producer.body if isinstance(node,ast.Assign)
                     and any(isinstance(target,ast.Name) and target.id=='WORKER_SOURCE' for target in node.targets)]
        self.assertEqual(len(assignments),1)
        expression=assignments[0]
        self.assertIsInstance(expression,ast.BinOp); self.assertIsInstance(expression.op,ast.Div)
        self.assertEqual(ast.dump(expression.left),ast.dump(ast.parse('HERE.parent',mode='eval').body))
        self.assertIsInstance(expression.right,ast.Constant)
        producer_relative=expression.right.value
        self.assertEqual(producer_relative,'candidate-v2/maintenance_fixture_worker.py')
        for filename,function in (('production_fixture_admission.py','validate_release'),
                                  ('production_fixture_history.py','check_setup')):
            source=ast.parse((a.HERE/filename).read_text())
            body=next(node for node in source.body if isinstance(node,ast.FunctionDef) and node.name==function)
            references=[node.value for node in ast.walk(body) if isinstance(node,ast.Constant)
                        and isinstance(node.value,str) and node.value.endswith('/maintenance_fixture_worker.py')]
            with self.subTest(reader=filename): self.assertEqual(references,[producer_relative])

    def test_only_exact_python_alias_prefix_is_permitted(self):
        python=self.root/'python'; python.write_bytes(b'inert')
        alias=self.root/'alias'; alias.symlink_to(python)
        family={str(python):reference(python)['sha256']}; expected=[str(alias),'script','--fixed']
        self.assertTrue(a.process_matches([str(python),'script','--fixed'],expected,family,str(alias)))
        self.assertFalse(a.process_matches([str(python),'script','--changed'],expected,family,str(alias)))
        self.assertFalse(a.process_matches([str(python),'script','--fixed','extra'],expected,family,str(alias)))

    def test_disabled_or_ambiguous_flag_cannot_transfer(self):
        path=self.root/'flag.py'
        for text in ('FLAG = False\n','FLAG = True\nFLAG = True\n','FLAG = bool(1)\n'):
            path.write_text(text)
            with self.assertRaisesRegex(ValueError,'preparatory'): a.assert_flag(a.Files(),path,'FLAG')
        path.write_text('FLAG = True\n'); a.assert_flag(a.Files(),path,'FLAG')

    def test_loaded_origin_cannot_substitute_other_core(self):
        core={str(a.HERE/(x+'.py')):'a'*64 for x in ('reservoir_maintenance','transaction_execution','recovery_ledger','recovery_phases','production_recovery_driver')}
        fixture={str(a.FIXTURE/(x+'.py')):'b'*64 for x in ('owned_fixture_coordinator','owned_recovery_core','owned_recovery_host','owned_fixture_records','owned_fixture_system')}
        release={'core':{'artifacts':core},'artifacts':fixture}
        record={'core_loaded_origins':core,'fixture_loaded_origins':fixture}; a.loaded_origins(record,release)
        record=copy.deepcopy(record); record['core_loaded_origins'][str(a.HERE/'reservoir_maintenance.py')]='c'*64
        with self.assertRaisesRegex(ValueError,'origin'): a.loaded_origins(record,release)


class ParentCloseout(Temporary):
    def proof(self):
        final=write(self.root/'final.json',{'known_identities':{'22':ident(22)},'sentinel':{'pid':23}})
        fixture={'parent':{'identity':ident()},'release':{'path':'/immutable/release.json','sha256':'a'*64},
                 'host_binding':{'uid':501,'boot_uuid':'boot'},'completed_wall_time_ns':1000000000,
                 'final_observations':final}
        fixture_ref=write(self.root/'fixture.json',fixture)
        raw=write(self.root/'raw.json',{'argv':['/bin/ps','-p','11','-o','pid='],'returncode':1,
                                      'stdout':'','stderr':'','observed_wall_time_ns':1500000000})
        closeout={'schema':'reservoir_owned_recovery_fixture_parent_closeout_v2','fixture':fixture_ref,
            'release':fixture['release'],'host_binding':fixture['host_binding'],'parent_identity':ident(),
            'observer_identity':ident(44),'parent_absent':True,'observation':{'queried_pid':11,'process':None},
            'observed_at':'1970-01-01T00:00:02+00:00','raw_observation':raw}
        review={'parent_closeout':write(self.root/'closeout.json',closeout)}
        return fixture_ref,fixture,review,closeout

    def test_independent_later_raw_absence_required(self):
        ref,fixture,review,_=self.proof(); a.check_closeout(a.Files(),ref,fixture,review)

    def test_self_or_owned_observer_refused(self):
        for pid in (11,22,23):
            ref,fixture,review,value=self.proof(); value['observer_identity']=ident(pid)
            review['parent_closeout']=write(self.root/'closeout.json',value)
            with self.subTest(pid=pid):
                with self.assertRaises(ValueError): a.check_closeout(a.Files(),ref,fixture,review)

    def test_stale_or_ambiguous_absence_refused(self):
        for change in ({'returncode':0},{'stderr':'permission denied'},{'observed_wall_time_ns':500000000}):
            ref,fixture,review,value=self.proof(); raw=json.loads(Path(value['raw_observation']['path']).read_text()); raw.update(change)
            value['raw_observation']=write(self.root/'raw.json',raw); review['parent_closeout']=write(self.root/'closeout.json',value)
            with self.subTest(change=change):
                with self.assertRaises(ValueError): a.check_closeout(a.Files(),ref,fixture,review)

    def test_legacy_passed_wrapper_refused_before_release_reads(self):
        fixture_ref=write(self.root/'legacy.json',{'schema':'reservoir_maintenance_launchd_fixture_v1','passed':True})
        review_ref=write(self.root/'review.json',{'accepted':True})
        with self.assertRaisesRegex(ValueError,'legacy'):
            a.validate_fixture({'fixture':fixture_ref,'fixture_review':review_ref},{'input_hashes':{'fixture':fixture_ref['sha256']}})


class Sessions(Temporary):
    def proof(self, child_end=False):
        self.plan={'transaction_id':'a'*32,'host_binding':{'uid':501,'boot_uuid':'boot'}}
        self.executor={'identity':ident(),'step':{'expected_exit':75,'kind':'resume_one'},
                       'self_start':{'started_wall_time_ns':10},'exit':{'completed_wall_time_ns':30}}
        self.rows=[]
        for index,name in enumerate(tuple('executor:'+s['id'] for s in a.schedule()[0] if s['kind']=='resume_one' and s['mode']=='reconcile' and s['action_id'].startswith('held_verified:'))+('cleanup_initialize','pin_initialize','finalize')):
            start=50+index*20
            claim={'type':'claim','action':name,'wall_time_ns':start,'executor_identity':ident(44)}
            result={'type':'result','action':name,'wall_time_ns':start+10,'claim_sha256':a.digest(claim),'result':{}}
            self.rows.extend((claim,result)); self.session(str(index+1)*32,ident(44),start+1,start+9)
        self.session('b'*32,ident(),20,29 if child_end else None)

    def session(self,sid,executor,start,end=None):
        begin={'schema':'reservoir_execution_session_v1','session_id':sid,'root':str(self.root),
            'transaction_id':self.plan['transaction_id'],'plan_sha256':a.digest(self.plan),'host_binding':self.plan['host_binding'],
            'mode':'resume_one','executor_identity':executor,'started_wall_time_ns':start}
        write(self.root/'execution-sessions'/(sid+'-begin.json'),begin)
        if end is not None:
            write(self.root/'execution-sessions'/(sid+'-end.json'),{
                'schema':'reservoir_execution_session_end_v1','session_id':sid,'begin_sha256':a.digest(begin),
                'transaction_id':self.plan['transaction_id'],'executor_identity':executor,
                'completed_wall_time_ns':end,'error_type':None,'external_action_success_inferred':False})

    def check(self):
        return h.check_sessions(a.Files(),self.root,self.plan,{'one':self.executor},ident(44),self.rows)

    def test_only_injected_natural_exit_allows_missing_session_end(self):
        self.proof(); self.check(); self.executor['step']['expected_exit']=0
        with self.assertRaisesRegex(ValueError,'unclosed'): self.check()

    def test_parent_missing_end_or_duplicate_phase_refused(self):
        self.proof(); end=self.root/'execution-sessions'/('1'*32+'-end.json'); saved=end.read_bytes(); end.unlink()
        with self.assertRaisesRegex(ValueError,'unclosed'): self.check()
        end.write_bytes(saved); self.session('c'*32,ident(44),52,58)
        with self.assertRaisesRegex(ValueError,'repeated'): self.check()

    def test_parent_outside_phase_or_phase_end_refused(self):
        self.proof(); self.session('1'*32,ident(44),51,61)
        with self.assertRaisesRegex(ValueError,'after reap or parent phase'): self.check()
        self.session('1'*32,ident(44),41,49)
        with self.assertRaisesRegex(ValueError,'outside exact phase'): self.check()

    def test_child_end_after_natural_reap_refused(self):
        self.proof(child_end=True); self.check(); self.session('b'*32,ident(),20,31)
        with self.assertRaisesRegex(ValueError,'after reap'): self.check()

    def test_orphan_end_and_unknown_session_file_refused(self):
        self.proof(); orphan=self.root/'execution-sessions'/('d'*32+'-end.json'); write(orphan,{})
        with self.assertRaisesRegex(ValueError,'orphan'): self.check()
        orphan.unlink(); write(self.root/'execution-sessions'/'extra.json',{})
        with self.assertRaisesRegex(ValueError,'unknown execution'): self.check()

    def test_exact_five_parent_sessions_required(self):
        self.proof()
        for suffix in ('begin','end'): (self.root/'execution-sessions'/('1'*32+'-'+suffix+'.json')).unlink()
        with self.assertRaisesRegex(ValueError,'five parent'): self.check()

    def test_foreign_executor_and_changed_boot_refused(self):
        self.proof(); self.session('b'*32,ident(99),20)
        with self.assertRaisesRegex(ValueError,'unknown executor'): self.check()
        self.session('b'*32,ident(),20); self.plan['host_binding']['boot_uuid']='changed'
        with self.assertRaisesRegex(ValueError,'shared executor'): self.check()


class InjectionBoundaries(Temporary):
    def test_host_exact_operation_role_and_interval(self):
        base=self.root/'executors/step'; point={'stream':'host','event':'host:debug_result','match':{'role':'provider'}}
        ref=write(self.root/'host-operation-00004.json',{'operation':'debug_result','role':'provider','wall_time_ns':20})
        h.check_injection_boundary(a.Files(),self.root,base,point,{'boundary':ref},10,30)
        for row in ({'operation':'signal_sent','role':'provider','wall_time_ns':20},
                    {'operation':'debug_result','role':'reservoir','wall_time_ns':20},
                    {'operation':'debug_result','role':'provider','wall_time_ns':5}):
            ref=write(self.root/'host-operation-00004.json',row)
            with self.subTest(row=row), self.assertRaises(ValueError):
                h.check_injection_boundary(a.Files(),self.root,base,point,{'boundary':ref},10,30)

    def test_pin_requires_exact_durable_file_not_any_retained_json(self):
        base=self.root/'executors/step'; point={'stream':'pin','event':'bootstrap_returned','match':{}}
        good=write(self.root/'pin/bootstrap-return.json',{'returncode':0})
        h.check_injection_boundary(a.Files(),self.root,base,point,{'boundary':good},10,30)
        wrong=write(self.root/'unrelated.json',{'returncode':0})
        with self.assertRaisesRegex(ValueError,'exact pin'):
            h.check_injection_boundary(a.Files(),self.root,base,point,{'boundary':wrong},10,30)


class Coordination(unittest.TestCase):
    def test_exact_recorded_owner_and_lifetime(self):
        release={'coordination':{'pause_generation':528,'actor':'codex-mlx-upgrade'}}
        value={'coordination_observation':{'coordination':release['coordination'],'observed_wall_time_ns':20}}
        a.check_coordination_observation(value,release,10,30)
        for change in ({'observed_wall_time_ns':5},{'coordination':{'pause_generation':527}},{'extra':True}):
            changed=copy.deepcopy(value); changed['coordination_observation'].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                a.check_coordination_observation(changed,release,10,30)

    def test_final_absence_cannot_reuse_prebootstrap_query(self):
        row={'returncode':1,'stdout':'','stderr':'','observed_wall_time_ns':25}
        a.check_query_window(row,20,30)
        for stamp in (1,19,31,True,None):
            with self.subTest(stamp=stamp), self.assertRaisesRegex(ValueError,'exact final phase'):
                a.check_query_window({**row,'observed_wall_time_ns':stamp},20,30)


class PinPrimitive(Temporary):
    def proof(self):
        root=self.root; pin=root/'pin'; pin.mkdir(exist_ok=True)
        python=root/'python'; python.write_bytes(b'inert, never executed')
        token='a'*32; target=f'gui/501/{a.PREFIX}{token}.pin'
        plan={'transaction_id':token,'host_binding':{'uid':501}}; release={'python':str(python),
            'python_files':{str(python):reference(python)['sha256']},'fixture':{'targets':{'pin':target}}}
        def settings(model):
            return {'Label':target.split('/')[-1],'ProgramArguments':[str(python),str(root/'owned_pin_sentinel.py'),
                '--root',str(root),'--token',token,'--model',str(pin/model),'--output',str(pin/'sentinel-exit-intent.json')],
                'WorkingDirectory':str(root),'RunAtLoad':True,'KeepAlive':False,
                'StandardOutPath':str(pin/'stdout'),'StandardErrorPath':str(pin/'stderr')}
        for name,model in (('original.plist','model-a.sentinel'),('candidate.plist','model-b.sentinel'),('active.plist','model-b.sentinel')):
            (pin/name).write_bytes(plistlib.dumps(settings(model)))
        for name in ('owned_pin_sentinel.py','owned_fixture_records.py'): (root/name).write_bytes(b'# inert source\n')
        binding={'token':token,'plan_sha256':a.digest(plan),'target':target,'scope':'private_pin_primitive_only'}
        header={'schema':'owned_fixture_ledger_v1','name':'pin','binding':binding}; write(pin/'header.json',header)
        inputs={'binding':binding,'original':reference(pin/'original.plist'),'candidate':reference(pin/'candidate.plist'),
                'sentinel':reference(root/'owned_pin_sentinel.py'),'record_helper':reference(root/'owned_fixture_records.py'),
                'production_pin_schedule_executed':False}; write(pin/'inputs.json',inputs)
        write(pin/'plist-replaced.json',{'active':reference(pin/'active.plist'),'candidate':inputs['candidate']})
        boot=write(pin/'bootstrap-return.json',{'target':target,'outcome':{'returncode':0,'argv':['/bin/launchctl','bootstrap','gui/501',str(pin/'active.plist')]}})
        write(pin/'remove-return.json',{'target':target,'outcome':{'returncode':0,'argv':['/bin/launchctl','bootout',target]}})
        sentinel=write(pin/'sentinel-exit-intent.json',{'token':token,'model_argument':str(pin/'model-b.sentinel'),
             'argv':settings('model-b.sentinel')['ProgramArguments'],'model_opened':False,'listener_opened':False,'numeric_imported':False,'pid':33})
        observation={'job_loaded':True,'process':None,'recorded_pid_absent':True,'last_exit_code':0,'runs':1,'listener_pids':[],
             'raw':{'argv':['/bin/launchctl','print',target],'returncode':0,'stdout':'runs = 1\nlast exit code = 0\n','stderr':''}}
        values={'replace_plist':{'active':reference(pin/'active.plist'),'applied':True},'bootstrap':{'bootstrapped':True,'return':boot},
            'verify_exit':{'passed':True,'receipt':sentinel,'observation':observation},'remove_absent_job':{'absent':True},
            'complete':{'passed':True,'production_pin_schedule_executed':False}}
        rows=[]; executors={}
        for action,result in values.items():
            rows=chain(header,[*rows,{'type':'claim','action':action,'executor_identity':ident()}]); claim=a.digest(rows[-1])
            rows=chain(header,[*rows,{'type':'result','action':action,'claim_sha256':claim,'result':result}])
            executors[action]={'step':{'kind':'pin','action_id':action,'mode':'execute','expected_exit':0},'identity':ident(),
                               'result':{'head_sha256':a.digest(rows[-1]),'result':result}}
        for i,row in enumerate(rows,1): write(pin/f'event-{i:05d}.json',row)
        return plan,release,executors

    def test_exact_private_model_only_pin_and_raw_natural_exit(self):
        plan,release,executors=self.proof(); h.check_pin(a.Files(),self.root,plan,release,executors)

    def test_pin_extra_setting_or_foreign_argv_rejected(self):
        plan,release,executors=self.proof(); path=self.root/'pin/candidate.plist'
        value=plistlib.loads(path.read_bytes()); value['KeepAlive']=True; path.write_bytes(plistlib.dumps(value))
        inputs=json.loads((self.root/'pin/inputs.json').read_text()); inputs['candidate']=reference(path); write(self.root/'pin/inputs.json',inputs)
        with self.assertRaises(ValueError): h.check_pin(a.Files(),self.root,plan,release,executors)

    def test_raw_sentinel_exit_nonzero_does_not_count_as_pass(self):
        plan,release,executors=self.proof()
        executors['verify_exit']['result']['result']['observation']['raw']['stdout']='runs = 1\nlast exit code = 9\n'
        # Keep the claim chain self-consistent: the independent raw predicate must reject it.
        header=json.loads((self.root/'pin/header.json').read_text()); rows=[]
        for action,executor in executors.items():
            rows=chain(header,[*rows,{'type':'claim','action':action,'executor_identity':ident()}]); claim=a.digest(rows[-1])
            rows=chain(header,[*rows,{'type':'result','action':action,'claim_sha256':claim,'result':executor['result']['result']}])
            executor['result']['head_sha256']=a.digest(rows[-1])
        for i,row in enumerate(rows,1): write(self.root/'pin'/f'event-{i:05d}.json',row)
        with self.assertRaisesRegex(ValueError,'raw natural exit'): h.check_pin(a.Files(),self.root,plan,release,executors)


class RecoveryChain(Temporary):
    """Exercise the complete pure chain; separate tests cover actual-effect inputs.

    These are explicitly synthetic records, never an accepted OS fixture. No
    producer/controller/Host module is imported to manufacture expected output.
    """
    def proof(self):
        token='a'*32; old={role:ident(10+i) for i,role in enumerate(a.ROLES)}
        holders={role:ident(20+i) for i,role in enumerate(a.ROLES)}
        replacements={role:ident(30+i) for i,role in enumerate(a.ROLES)}
        plan={'transaction_id':token,'actor':'owned-fixture-only','host_binding':{'uid':501,'boot_uuid':'boot'},
              'roles':{role:{'identity':old[role]} for role in a.ROLES},
              'input_hashes':{'reservoir_candidate':'c'*64,'interpreter':'d'*64,'reservoir_settings':'e'*64}}
        spec={'plan':plan,'transaction_root':str(self.root),'execution_scope':'owned_fixture'}
        guard={'schema':'model_environment_hold_v1','transaction_id':token,'actor':plan['actor']}
        original=[{'type':'preflight','receipt':{'passed':True,'execution_scope':'owned_fixture'}},
            {'type':'mutation_intent','operation':'create_hold','role':'provider'},
            {'type':'mutation_completed','operation':'create_hold','role':'provider','result':{'guard':guard}},
            {'type':'mutation_intent','operation':'arm_next_spawn','role':'provider'}]
        previous=a.digest(plan)
        for index,row in enumerate(original,1):
            row.update(sequence=index,previous_sha256=previous,plan_sha256=a.digest(plan)); previous=a.digest(row)
            write(self.root/'controller'/f'event-{index:05d}.json',row)
        write(self.root/'controller/plan.json',plan)
        header={'schema':'reservoir_recovery_ledger_v1','root':str(self.root),'transaction_id':token,
                'plan_sha256':a.digest(plan),'preparation_sha256':a.digest(spec),'original_controller_head_sha256':previous}
        write(self.root/'recovery/header.json',header)
        rows=[]; receipts={}; pending=[]; sessions={}; executors={}
        for number,step in enumerate([s for s in a.schedule()[0] if s['kind']=='resume_one'],1):
            action=step['action_id']; op,_,role=action.partition(':'); role=role or None
            ident_value=ident(100+number); session={'executor_identity':ident_value,'session_id':f'{number:032x}'}
            sessions[a.digest(session)]=session
            bindings={'plan_sha256':a.digest(plan),**{r+'_old_identity_sha256':a.digest(old[r]) for r in a.ROLES}}
            if action=='witness:backup': bindings['final_snapshot_sha256']=a.digest(receipts['final_snapshot'])
            if action=='witness:private_restore': bindings.update(backup_sha256=a.digest(receipts['backup']),
                repair_source_sha256='c'*64,interpreter_sha256='d'*64,configuration_sha256='e'*64)
            if action=='witness:startup': bindings.update(private_restore_sha256=a.digest(receipts['private_restore']),reservoir_new_identity_sha256=a.digest(replacements['reservoir']))
            if action=='witness:provider_ready': bindings.update(startup_sha256=a.digest(receipts['startup']),provider_new_identity_sha256=a.digest(replacements['provider']))
            details={'action_id':action,'operation':None if op=='witness' else op,'role':None if op=='witness' else role,
                'witness':role if op=='witness' else None,'context':{'transaction_id':token,'plan':plan,'bindings':bindings,
                'execution_scope':'owned_fixture','production_qualified':False,'receipts':copy.deepcopy(receipts)},
                'checkpoint_sha256':'f'*64,'inspection_sha256':'9'*64,'host_before_count':0,'host_before_sha256':a.digest([])}
            before=a.digest(rows[-1]) if rows else a.digest(plan)
            def append(kind,**extra):
                row={'schema':'reservoir_recovery_event_v1','sequence':len(rows)+1,
                     'previous_sha256':a.digest(rows[-1]) if rows else a.digest(header),'transaction_id':token,
                     'plan_sha256':a.digest(plan),'type':kind,'executor_session':session,**copy.deepcopy(details),**copy.deepcopy(extra)}
                rows.append(row); return row
            claim=append('action_claim' if step['mode']=='execute' else 'reconciliation_claim'); pending.append(a.digest(claim))
            executor={'identity':ident_value,'step':step,'request':{'expected_heads':{
                'controller_head_sha256':previous,'host_operations_sha256':a.digest([]),'recovery_head_sha256':before,'inspection_sha256':'9'*64}}}
            executors[step['id']]=executor
            if step['expected_exit']==75: continue
            if op=='witness':
                result={'schema':'reservoir_maintenance_witness_v1','kind':role,'transaction_id':token,'passed':True,
                        'bindings':bindings,'evidence':{'scope':'owned_fixture_only','production_qualified':False}}
                receipts[role]=result; write(self.root/(role+'-witness.json'),result)
            elif op=='create_hold': result={'guard':guard}
            elif op=='arm_next_spawn': result={'armed':True}
            elif op=='signal_stop': result={'signal':'SIGTERM','identity':old[role],'force_used':False}
            elif op=='held_verified': result={'identity':holders[role]}
            elif op=='remove_held': result={'removed_identity':holders[role]}
            elif op=='job_absent': result={'absent':True}
            elif op=='install_repair': result={'source_sha256':'c'*64}
            elif op=='bootstrap_original': result={'identity':replacements[role]}
            elif op=='release_hold': result={'released':True}
            else: result={'schema':'reservoir_maintenance_result_v1','transaction_id':token,'status':'verified','plan_sha256':a.digest(plan),'witness_hashes':{k:a.digest(v) for k,v in receipts.items()},
                          **replacements,'force_used':False,'runtime_state_rollback':False,'execution_scope':'owned_fixture'}
            extra={'claim_sha256':a.digest(claim),'result':result,'host_operations':{'before_count':0,'after_count':0,
                         'before_sha256':a.digest([]),'after_sha256':a.digest([]),'appended':[]}}
            if step['mode']=='reconcile':
                extra.update(resolves_claims=list(pending),reconciliation={'schema':'reservoir_recovery_reconciliation_v1',
                    'passed':True,'action_id':action,'transaction_id':token,'plan_sha256':a.digest(plan),
                    'checkpoint_sha256':'f'*64,'inspection_sha256':'9'*64,'result':result,'evidence':{}})
            row=append('action_result' if step['mode']=='execute' else 'reconciliation_result',**extra); pending=[]
            executor['result']={'schema':'reservoir_recovery_step_v1','action_id':action,'record_sha256':a.digest(row),
                               'recovery_head_sha256':a.digest(row),'result':result,'automatic_next_action':False}
        for index,row in enumerate(rows,1): write(self.root/'recovery'/f'event-{index:05d}.json',row)
        return plan,spec,executors,sessions

    def test_complete_chain_preserves_all_context_and_reaps(self):
        plan,spec,executors,sessions=self.proof()
        result=h.check_core_history(a.Files(),self.root,plan,spec,executors,sessions)
        self.assertEqual(list(result['receipts']),list(a.WITNESSES))
        self.assertEqual(set(result['replacements']),set(a.ROLES))

    def test_stale_requested_head_and_context_cannot_be_adopted(self):
        plan,spec,executors,sessions=self.proof()
        first=next(iter(executors.values())); first['request']['expected_heads']['recovery_head_sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'stale/different'): h.check_core_history(a.Files(),self.root,plan,spec,executors,sessions)

    def test_unknown_host_span_cannot_become_completed_receipt(self):
        plan,spec,executors,sessions=self.proof()
        path=self.root/'recovery/event-00002.json'; row=json.loads(path.read_text()); row['host_operations']['after_count']=1
        write(path,row)
        with self.assertRaisesRegex(ValueError,'Host span'): h.check_core_history(a.Files(),self.root,plan,spec,executors,sessions)


if __name__ == '__main__': unittest.main()
