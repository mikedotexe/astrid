"""Temporary fake evidence only; no actual numerical/service proof is accepted or created."""
from contextlib import contextmanager
import copy
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import production_admission as a
import reservoir_maintenance as m


class ReaderBoundaryTests(unittest.TestCase):
    def test_aliases_and_import_path_restored_on_success_and_error(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'first.py').write_text('VALUE = 7\n')
            (root/'second.py').write_text('import first\nVALUE = first.VALUE\n')
            bound={str(p):a.file_hash(p) for p in root.glob('*.py')}
            prior=SimpleNamespace(VALUE='foreign');original_path=list(sys.path)
            with patch.dict(sys.modules,{'first':prior}):
                for fail in (False,True):
                    try:
                        with a.modules(root,('first','second'),bound) as loaded:
                            self.assertEqual(loaded['second'].VALUE,7)
                            self.assertIs(sys.modules['first'],loaded['first'])
                            if fail:raise RuntimeError('fake reader exception')
                    except RuntimeError:self.assertTrue(fail)
                    self.assertIs(sys.modules['first'],prior);self.assertNotIn('second',sys.modules)
                    self.assertEqual(sys.path,original_path)
            self.assertFalse((root/'__pycache__').exists())
    def test_reader_must_be_bound_before_import(self):
        with tempfile.TemporaryDirectory() as temp:
            root=Path(temp);(root/'reader.py').write_text('raise AssertionError("must never run")\n')
            with self.assertRaisesRegex(m.Refusal,'unbound'):
                with a.modules(root,('reader',),{}):pass
    def test_boolean_service_or_restore_summary_cannot_pass(self):
        with self.assertRaisesRegex(m.Refusal,'matrix review'):a.validate_service_matrix({}, {'passed':True}, {})
        with self.assertRaisesRegex(m.Refusal,'nine-case'):a.validate_restore_result({}, {'passed':True}, {})
    def test_foreign_restore_reader_alias_refuses_before_authority_call(self):
        foreign=SimpleNamespace(__file__='/foreign/production_restore_admission.py',validate_chain=Mock())
        with patch.object(a,'restore_admission',foreign):
            with self.assertRaisesRegex(m.Refusal,'different source'):
                a.validate_restore_result({}, {}, {})
            with self.assertRaisesRegex(m.Refusal,'different source'):
                a.validate_service_history({}, {}, [], {})
        foreign.validate_chain.assert_not_called()
    def test_no_numerical_modules_imported(self):
        for name in ('numpy','torch','mlx','coremltools','reservoir_service'):self.assertNotIn(name,sys.modules)


class ChildTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve();self.argv=['/fake/python','-B','/fake/reader.py']
        for name in ('stdout.txt','stderr.txt'):(self.root/name).write_text('SYNTHETIC TEST\n')
        self.claim=self.root/'claim.json';self.claim.write_text('{}')
        self.row={'admission_claim':a.ref(self.claim),'pid':42,'argv':self.argv,'started_wall':2.0,'completed_wall':3.0,'reaped':True,'returncode':0,
                  'stdout':a.ref(self.root/'stdout.txt'),'stderr':a.ref(self.root/'stderr.txt')}
        self.save()
    def save(self):
        (self.root/'child-exit.json').write_text(json.dumps(self.row))
        (self.root/'child-start.json').write_text(json.dumps({'pid':self.row['pid'],'argv':self.row['argv'],
            'started_wall':self.row['started_wall'],'scope':'owned_synthetic_cpu_child','signals_permitted':False,
            'admission_claim':self.row['admission_claim']}))
    def validate(self):a.validate_child(self.row,self.root,self.argv,0,1,4,a.ref(self.claim))
    def test_natural_exit_and_exact_retained_records(self):self.validate()
    def test_wrong_exit_pid_argv_overlap_reaping_fail(self):
        for key,value in [('returncode',1),('pid',1),('argv',['wrong']),('reaped',False),('started_wall',.5),('completed_wall',5)]:
            original=copy.deepcopy(self.row);self.row[key]=value;self.save()
            with self.subTest(key=key),self.assertRaises(m.Refusal):self.validate()
            self.row=original;self.save()
    def test_changed_stream_or_unresolved_child_refuses(self):
        (self.root/'stdout.txt').write_text('changed')
        with self.assertRaisesRegex(m.Refusal,'stream hash'):self.validate()
        self.row['stdout']=a.ref(self.root/'stdout.txt');self.save()
        (self.root/'child-unresolved.json').write_text('{}')
        with self.assertRaisesRegex(m.Refusal,'unresolved'):self.validate()
    def test_missing_or_altered_exit_receipt_refuses(self):
        (self.root/'child-exit.json').write_text('{}')
        with self.assertRaisesRegex(m.Refusal,'exit receipt'):self.validate()


class RestoreClosureTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve();self.harness=self.root/'qualification';self.harness.mkdir()
        self.prod=self.root/'production';self.prod.mkdir();self.run=self.harness/'runs/fake-run';self.run.mkdir(parents=True)
        self.patchers=[patch.object(a,'RESTORE_HARNESS',self.harness),patch.object(a,'HERE',self.prod),
                       patch.object(a.restore_admission,'__file__',str(self.prod/'production_restore_admission.py'))]
        for item in self.patchers:item.start();self.addCleanup(item.stop)
        for name in ('restore_verifier.py','reservoir_maintenance_state.py'):
            (self.harness/name).write_text('# synthetic helper never imported\n');(self.prod/name).write_text('# synthetic helper never imported\n')
        self.artifacts={str(p):a.file_hash(p) for p in self.harness.glob('*.py')}
        self.runtime={'python':'/fake/original/python','interpreter_files':{'/fake/python':'a'*64},'packages':{'numpy':{'version':'fake'}}}
        self.source={'root':str(self.harness/'source'),'files':{'persistence.py':'b'*64}}
        self.config={'input_dim':32,'n_nodes':192,'backend':'numpy'}
        self.inputs={'runtime_manifest':self.runtime,'source_manifest':self.source,'config':self.config,'artifacts':{'/fake/input':'c'*64}}
        inputs_ref=self.write(self.harness/'inputs.json',self.inputs)
        raw_ref=self.write(self.root/'host.json',{'synthetic':True});gate_ref=self.write(self.root/'gate.json',{'synthetic':True})
        release={'schema':'reservoir_restore_qualification_release_v2','accepted':True,'reviewer':'synthetic unit test',
                 'scope':'synthetic_cpu_fixture_only','purpose':'release_restore_qualification_cpu','run_id':'fake-run',
                 'inputs':inputs_ref,'artifacts':self.artifacts,'input_artifacts':self.inputs['artifacts'],
                 'expires_wall':500,'host_gate_raw':raw_ref,'host_gate_review':gate_ref}
        self.release_ref=self.write(self.root/'release.json',release)
        self.seed={'schema':'reservoir_restore_seed_v1','passed':True,'production_qualified':False,
                   'scope':'synthetic_cpu_fixture_only','pid':100,'python':self.runtime['python'],'inputs':inputs_ref,
                   'errors':[],'tasks_started':False,'listener_opened':False,'torch_device':'cpu','torch_threads':1,
                   'expected_startup':{'qualification_numpy':{'backend':'numpy'},'qualification_shadow__lsm':{'backend':'lsm_shadow'}},
                   'inventory':{'synthetic':'inventory'}}
        seed_ref=self.write(self.run/'seed/seed-receipt.json',self.seed)
        seed_argv=[self.runtime['python'],'-B',str(self.harness/'seed_fixture.py'),'--execute-fixture',
                   '--output',str(self.run/'seed'),'--inputs',inputs_ref['path']]
        seed_child=self.child(self.run/'seed',seed_argv,100,1,2,0)
        self.result={'schema':'reservoir_restore_qualification_v1','passed':True,'scope':'synthetic_cpu_fixture_only',
                     'production_qualified':False,'errors':[],'final_runtime_and_positive_witness_revalidated':True,
                     'started_wall':0,'completed_wall':100,'inputs':inputs_ref,'release':self.release_ref,'seed':seed_ref,
                     'seed_child':seed_child,'artifacts':self.artifacts,'cases':[]}
        self.classifications={}
        for index,case in enumerate(a.RESTORE_CASES):
            directory=self.run/case;work=self.harness/'restore-runs'/('fixture-fake-run-'+case)
            context={'synthetic':case};runtime=copy.deepcopy(self.runtime);source=copy.deepcopy(self.source);config=dict(self.config)
            if case=='package-drift':runtime['packages']['numpy']['version']='0.0.0-private-negative-control'
            if case=='source-drift':source['root']=str(directory/'source')
            if case=='config-mismatch':config['n_nodes']=64
            request={'context':context,'bindings':{'test':case},'runtime_manifest':runtime,'source_manifest':source,'config':config,
                     'work_root':str(work),'backup_witness':{'evidence':{'backup_root':str(directory/'backup')}}}
            request_ref=self.write(directory/'request.json',request)
            argv=[self.runtime['python'],'-B',str(self.harness/'restore_verifier.py'),'--request',request_ref['path'],'--execute-private-restore']
            child=self.child(directory,argv,101+index,3+index*3,5+index*3,0 if index==0 else 1)
            classification={'case':case,'passed':True,'expected_rejection':case!='complete'}
            if case!='complete':classification['reason']='SYNTHETIC EXPECTED ERROR'
            if case=='config-mismatch':classification['witness']=None
            else:
                cp=self.write(work/'comparison.json',{'pid':child['pid'],'argv':argv[2:],
                               'started_wall':child['started_wall']+.1,'completed_wall':child['completed_wall']-.1})
                log=work/'restore.log';log.write_text('SYNTHETIC\n')
                classification['witness']=self.write(work/'restore-witness.json',{'bindings':request['bindings'],
                    'evidence':{'request':request_ref,'comparison':cp,'log':a.ref(log)}})
            self.classifications[case]=copy.deepcopy(classification)
            row=dict(classification,request=request_ref,child=child);self.write(directory/'expected-result.json',row)
            self.result['cases'].append(row)
        self.spec={'restore_qualification':self.write(self.run/'qualification.json',self.result),
                   'restore_runtime_manifest':self.runtime,'restore_source_manifest':self.source,
                   'reservoir_config':dict(self.config,backend_label='NumPy')}
        self.write(self.harness/'launch-guard.json',{'run_id':'fake-run','release':self.release_ref,
                   'artifacts':self.artifacts,'automatic_retry_permitted':False})
        self.write(self.run/'seed/seed-execution-guard.json',{'run_id':'fake-run','pid':100,
                   'scope':'synthetic_cpu_fixture_only','automatic_retry_permitted':False})
        chain_patch=patch.object(a.restore_admission,'validate_chain',return_value={'claims':{
            'seed':seed_child['admission_claim'],**{row['case']:row['child']['admission_claim'] for row in self.result['cases']}}})
        chain_patch.start();self.addCleanup(chain_patch.stop)
        self.classified=[]
        def classify(v,case,request,child,directory,seed):self.classified.append(case);return self.classifications[case]
        q=SimpleNamespace(check=lambda:self.inputs,harness_hashes=lambda:self.artifacts,
                          validate_host_gate=lambda *args,**kwargs:None,context_for=lambda v,i,r,c:{'synthetic':c},classify_case=classify)
        v=SimpleNamespace(RUNS=self.harness/'restore-runs',validate_request=lambda value:value,
                          state=SimpleNamespace(inventory_tree=lambda path:self.seed['inventory']))
        @contextmanager
        def fake_modules(*args):yield {'qualify_restore':q,'restore_verifier':v}
        self.modules_patch=patch.object(a,'modules',fake_modules);self.modules_patch.start();self.addCleanup(self.modules_patch.stop)
        # Input byte preservation has separate real-stdlib fake-tree tests below.
        input_patch=patch.object(a,'validate_case_inputs');input_patch.start();self.addCleanup(input_patch.stop)
    def write(self,path,value):
        path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value));return a.ref(path)
    def child(self,directory,argv,pid,start,end,code):
        directory.mkdir(parents=True,exist_ok=True)
        for name in ('stdout.txt','stderr.txt'):(directory/name).write_text('SYNTHETIC\n')
        claim=self.write(directory/'fake-claim.json',{'synthetic':pid})
        row={'admission_claim':claim,'pid':pid,'argv':argv,'started_wall':start,'completed_wall':end,'returncode':code,'reaped':True,
             'stdout':a.ref(directory/'stdout.txt'),'stderr':a.ref(directory/'stderr.txt')}
        self.write(directory/'child-start.json',{'pid':pid,'argv':argv,'started_wall':start,
                                                'scope':'owned_synthetic_cpu_child','signals_permitted':False,'admission_claim':claim})
        self.write(directory/'child-exit.json',row);return row
    def validate(self):return a.validate_restore_result(self.spec,self.result,self.artifacts)
    def test_full_synthetic_shape_revalidates_all_nine_cases(self):
        self.assertEqual(self.validate()['case_count'],9);self.assertEqual(self.classified,list(a.RESTORE_CASES))
    def test_missing_duplicate_or_reordered_case_refuses(self):
        original=copy.deepcopy(self.result['cases'])
        for rows in (original[:-1],original+[original[0]],list(reversed(original))):
            self.result['cases']=rows
            with self.assertRaisesRegex(m.Refusal,'ordered nine'):self.validate()
        self.result['cases']=original
    def test_bad_scope_or_no_final_revalidation_refuses(self):
        for key,value in [('production_qualified',True),('errors',['failure']),('final_runtime_and_positive_witness_revalidated',False)]:
            old=self.result[key];self.result[key]=value
            with self.subTest(key=key),self.assertRaises(m.Refusal):self.validate()
            self.result[key]=old
    def test_different_installed_runtime_or_source_refuses(self):
        for key in ('restore_runtime_manifest','restore_source_manifest'):
            old=self.spec[key];self.spec[key]={} if key=='restore_runtime_manifest' else {'files':{}}
            with self.subTest(key=key),self.assertRaisesRegex(m.Refusal,'different runtime'):self.validate()
            self.spec[key]=old
    def test_wrong_positive_child_or_missing_raw_stream_refuses(self):
        child=self.result['cases'][0]['child'];child['returncode']=1
        with self.assertRaisesRegex(m.Refusal,'child exit'):self.validate()
        child['returncode']=0;Path(child['stdout']['path']).unlink()
        with self.assertRaises(FileNotFoundError):self.validate()
    def test_exact_child_pid_bound_to_restore_comparison(self):
        row=self.result['cases'][0];witness=a.reference(row['witness']);comparison=a.reference(witness['evidence']['comparison'])
        comparison['pid']=999;witness['evidence']['comparison']=self.write(Path(witness['evidence']['comparison']['path']),comparison)
        row['witness']=self.write(Path(row['witness']['path']),witness);self.classifications['complete']['witness']=row['witness']
        with self.assertRaisesRegex(m.Refusal,'exact child'):self.validate()
    def test_expired_historical_launch_refuses_but_old_completion_is_allowed(self):
        self.validate()  # synthetic wall times are old; no present-time expiry is invented.
        release=a.reference(self.release_ref);release['expires_wall']=1.5
        self.result['release']=self.write(Path(self.release_ref['path']),release)
        self.write(self.harness/'launch-guard.json',{'run_id':'fake-run','release':self.result['release'],
                   'artifacts':self.artifacts,'automatic_retry_permitted':False})
        with self.assertRaisesRegex(m.Refusal,'after historical release expiry'):self.validate()


class RetainedInputTests(unittest.TestCase):
    def setUp(self):
        import restore_verifier
        self.v=restore_verifier
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name).resolve()
        (self.root/'backup').mkdir();(self.root/'backup/fake.npz').write_bytes(b'not a numeric file')
        self.request={'backup_witness':{'evidence':{'backup_inventory':self.v.state.inventory_tree(self.root/'backup')}}}
        self.inputs={'source_manifest':{'root':str(self.root/'original'),'files':{'persistence.py':'unused','support.py':'unused'}}}
        (self.root/'original').mkdir();(self.root/'source').mkdir()
        for name in self.inputs['source_manifest']['files']:
            (self.root/'original'/name).write_bytes(b'# synthetic source never imported\n')
            (self.root/'source'/name).write_bytes((self.root/'original'/name).read_bytes())
        with (self.root/'source/persistence.py').open('ab') as stream:stream.write(b'\n# intentional private drift, never imported\n')
    def test_negative_backup_hashes_remain_bound(self):
        a.validate_case_inputs(self.v,self.inputs,self.request,'corrupt-npz',self.root)
        (self.root/'backup/fake.npz').write_bytes(b'changed after run')
        with self.assertRaisesRegex(m.Refusal,'backup changed'):a.validate_case_inputs(self.v,self.inputs,self.request,'corrupt-npz',self.root)
    def test_backup_drift_accepts_only_exact_extra_file(self):
        (self.root/'backup/intentional-private-drift.txt').write_bytes(b'not in bound inventory\n')
        a.validate_case_inputs(self.v,self.inputs,self.request,'backup-drift',self.root)
        (self.root/'backup/another.txt').write_bytes(b'extra')
        with self.assertRaisesRegex(m.Refusal,'beyond named'):a.validate_case_inputs(self.v,self.inputs,self.request,'backup-drift',self.root)
    def test_source_drift_exact_comment_and_other_sources_preserved(self):
        a.validate_case_inputs(self.v,self.inputs,self.request,'source-drift',self.root)
        (self.root/'source/support.py').write_bytes(b'changed support')
        with self.assertRaisesRegex(m.Refusal,'source drift'):a.validate_case_inputs(self.v,self.inputs,self.request,'source-drift',self.root)


class FixtureProofTests(unittest.TestCase):
    def test_delegates_only_to_exact_final_fixture_reader(self):
        reader=SimpleNamespace(__file__=str(a.HERE/'production_fixture_admission.py'),validate_fixture=Mock(return_value={'schema':'normalized-v2'}))
        spec={'fixture':{'legacy':'must not be accepted here'}};plan={'fake':'plan'}
        with patch.dict(sys.modules,{'production_fixture_admission':reader}):
            self.assertEqual(a.validate_fixture(spec,plan),{'schema':'normalized-v2'})
        reader.validate_fixture.assert_called_once_with(spec,plan)
    def test_reader_refusal_cannot_fall_back_to_legacy_success(self):
        reader=SimpleNamespace(__file__=str(a.HERE/'production_fixture_admission.py'),validate_fixture=Mock(side_effect=m.Refusal('legacy fixture refused')))
        with patch.dict(sys.modules,{'production_fixture_admission':reader}):
            with self.assertRaisesRegex(m.Refusal,'legacy fixture refused'):
                a.validate_fixture({'passed':True},{})


    def test_preloaded_foreign_reader_alias_is_rejected_before_call(self):
        reader=SimpleNamespace(__file__='/foreign/production_fixture_admission.py',validate_fixture=Mock())
        with patch.dict(sys.modules,{'production_fixture_admission':reader}):
            with self.assertRaisesRegex(m.Refusal,'different source'):
                a.validate_fixture({}, {})
        reader.validate_fixture.assert_not_called()


class MatrixBindingTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name).resolve()
        self.source={'reservoir_service.py':'a'*64,'persistence.py':'b'*64}
        self.runtime={'python':'/fake/python','interpreter_files':{'/fake/python':'c'*64},
                      'packages':{name:{'version':'fake','files':{'/fake/'+name+'/METADATA':'d'*64,'/fake/'+name+'/RECORD':'e'*64}}
                                  for name in ('numpy','torch','scikit-learn','websockets')}}
        original=dict(self.source);original['reservoir_service.py']='f'*64
        self.manifest={'variants':{'original':original,'candidate':self.source},
                       'expected_interpreter':{'executable':'/fake/python','files':self.runtime['interpreter_files']},
                       'expected_packages':{name:'fake' for name in self.runtime['packages']}}
        self.manifest_ref=self.write(self.root/'sources.json',self.manifest)
        refs=[]
        for i in range(14):
            row={'manifest':self.manifest_ref['path'],'manifest_sha256':self.manifest_ref['sha256'],
                 'interpreter':self.manifest['expected_interpreter']}
            refs.append(self.write(self.root/str(i)/'result.json',row))
            self.write(self.root/str(i)/'child.json',{'distribution_files':{name:row['files'] for name,row in self.runtime['packages'].items()}})
        self.review={'schema':'reservoir_private_matrix_review_v1','accepted_for_private_numpy_only':True,
                     'deployment_accepted':False,'mlx_upgrade_qualified':False,'receipts':refs}
        self.spec={'restore_runtime_manifest':self.runtime,'restore_source_manifest':{'files':self.source},
                   'plan':{'input_hashes':{'reservoir_source':'f'*64}}}
        self.rereview=copy.deepcopy(self.review)
        hp=patch.object(a,'validate_service_history',return_value={'synthetic':'tested separately'});hp.start();self.addCleanup(hp.stop)
        @contextmanager
        def fake_modules(*args):yield {'review_matrix':SimpleNamespace(review=lambda paths:self.rereview)}
        patcher=patch.object(a,'modules',fake_modules);patcher.start();self.addCleanup(patcher.stop)
    def write(self,path,value):
        path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value));return a.ref(path)
    def test_source_interpreter_and_all_four_cpu_package_artifacts_bound(self):
        self.assertEqual(a.validate_service_matrix(self.spec,self.review,{})['case_count'],14)
    def test_partial_or_falsely_renamed_success_refuses(self):
        for change in (lambda r:r.update(receipts=r['receipts'][:-1]),lambda r:r.update(deployment_accepted=True)):
            value=copy.deepcopy(self.review);change(value)
            with self.assertRaises(m.Refusal):a.validate_service_matrix(self.spec,value,{})
    def test_raw_reviewer_disagreement_refuses(self):
        self.rereview['accepted_for_private_numpy_only']=False
        with self.assertRaisesRegex(m.Refusal,'raw re-review'):a.validate_service_matrix(self.spec,self.review,{})
    def test_different_prepared_source_or_original_runtime_refuses(self):
        self.spec['restore_source_manifest']={'files':dict(self.source,**{'persistence.py':'0'*64})}
        with self.assertRaisesRegex(m.Refusal,'source closure'):a.validate_service_matrix(self.spec,self.review,{})
        self.spec['restore_source_manifest']={'files':self.source};self.runtime['interpreter_files']={'/other/python':'0'*64}
        with self.assertRaisesRegex(m.Refusal,'interpreter differs'):a.validate_service_matrix(self.spec,self.review,{})
    def test_changed_distribution_artifact_refuses(self):
        p=self.root/'0/child.json';row=json.loads(p.read_text());row['distribution_files']['numpy']['/fake/numpy/RECORD']='0'*64
        self.write(p,row)
        with self.assertRaisesRegex(m.Refusal,'package bytes'):a.validate_service_matrix(self.spec,self.review,{})


class AdmissionEnvelopeTests(unittest.TestCase):
    def setUp(self):
        import test_production_controller as c
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name).resolve()
        self.plan=c.legacy.fake_plan();self.plan['roles']['provider']['identity']['argv']+=['--model','hub/id']
        self.plan['input_hashes']['provider_restart_settings']='a'*64;self.plan=m.validate_plan(self.plan)
        self.service=self.root/'service';self.service.mkdir();self.restore=self.root/'restore';self.restore.mkdir()
        self.hold=self.root/'hold.py';self.hold.write_text('# synthetic never executed')
        for name in a.RUNTIME_HELPERS:(self.root/name).write_text('# synthetic never imported')
        for name in ('qualify_restore.py','seed_fixture.py','test_qualification.py','restore_verifier.py','reservoir_maintenance_state.py','child_admission.py','test_child_admission.py'):
            (self.restore/name).write_text('# synthetic never imported')
        (self.service/'reader.py').write_text('# synthetic never imported')
        artifacts={str(path):a.file_hash(path) for path in self.root.rglob('*.py')}
        self.spec={'schema':'reservoir_production_preparation_v1','plan':self.plan,'immutable_files':artifacts}
        for key in ('fixture','fixture_review','real_matrix','restore_qualification','service_root_acceptance','restore_root_acceptance'):
            self.spec[key]=self.write(key+'.json',{'synthetic':key})
        self.plan['input_hashes']['fixture']=self.spec['fixture']['sha256']
        self.spec['original_provider_manifest']=self.write('model.json',{'argv':['--model','/fake/snapshot']})
        self.spec['model_resolution']=self.write('resolution.json',{'schema':'original_provider_model_resolution_review_v1',
            'accepted':True,'snapshot':'/fake/snapshot','original_argv':self.plan['roles']['provider']['identity']['argv'],
            'reviewer':'synthetic unit test','restart_policy':'pin','evidence':['synthetic']})
        self.spec['provider_restart_argv']=a.p.pinned_argv(self.plan['roles']['provider']['identity']['argv'],'/fake/snapshot')
        plist=self.root/'restart.plist';plist.write_text('synthetic bytes');self.spec['provider_restart_plist']=a.ref(plist)
        self.plan['input_hashes']['provider_restart_settings']=a.file_hash(plist)
        self.review={'schema':'reservoir_production_execution_review_v1','approved':True,'reviewer':'synthetic unit test',
                     'artifacts':artifacts,**{key:self.spec[key] for key in ('fixture','fixture_review','real_matrix','restore_qualification','service_root_acceptance','restore_root_acceptance')}}
        self.spec['execution_review']={'path':str(self.root/'review.json'),'sha256':'0'*64};self.save_review()
        patches=[patch.object(a,'HERE',self.root),patch.object(a,'SERVICE_HARNESS',self.service),patch.object(a,'RESTORE_HARNESS',self.restore),
                 patch.object(a.p,'HOLD_SOURCE',self.hold),patch.object(a.p,'load_hold',return_value=SimpleNamespace(option=lambda argv,key:argv[argv.index(key)+1])),
                 patch.object(a,'validate_fixture'),patch.object(a,'validate_service_matrix'),patch.object(a,'validate_restore_result')]
        self.validators=[]
        for item in patches:
            value=item.start();self.addCleanup(item.stop);self.validators.append(value)
    def write(self,name,value):
        path=self.root/name;path.write_text(json.dumps(value));return a.ref(path)
    def save_review(self):
        self.review['preparation_sha256']=a.p.preparation_digest(self.spec)
        self.spec['execution_review']=self.write('review.json',self.review)
    def test_exact_bound_envelope_invokes_all_raw_validators(self):
        result=a.validate_admission(self.spec,self.plan)
        m.validate_production_admission(self.plan,result)
        for validator in self.validators[-3:]:validator.assert_called_once()
    def test_missing_or_changed_review_binding_refuses(self):
        self.review['fixture_review']={'path':'/other','sha256':'f'*64};self.save_review()
        with self.assertRaisesRegex(m.Refusal,'reviewed preparation'):a.validate_admission(self.spec,self.plan)
    def test_integrated_helper_drift_refuses_before_proof_readers(self):
        (self.root/'production_recovery.py').write_text('changed')
        with self.assertRaisesRegex(m.Refusal,'artifact changed'):a.validate_admission(self.spec,self.plan)
        for validator in self.validators[-3:]:validator.assert_not_called()
    def test_no_proof_can_substitute_another_model_path(self):
        self.spec['provider_restart_argv'][-1]='/other';self.save_review()
        with self.assertRaisesRegex(m.Refusal,'provider substitution'):a.validate_admission(self.spec,self.plan)

if __name__=='__main__':unittest.main(verbosity=2)
