"""Temporary synthetic records only; no numerical inputs, workload or host sampling."""
from contextlib import ExitStack
import copy
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import production_admission as a
import production_restore_admission as r
import reservoir_maintenance as m


class RestoreHistoryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve(); self.run = self.root / 'runs/fake-run'
        self.run.mkdir(parents=True); self.stack = ExitStack(); self.addCleanup(self.stack.close)
        # Only the two reviewed stdlib readers are loaded, never q.check/numerics.
        real = a.RESTORE_HARNESS
        self.ca = self.stack.enter_context(a.modules(real, ('child_admission',),
                         {str(real / 'child_admission.py'): a.file_hash(real / 'child_admission.py')}))['child_admission']
        policy_path = real.parent.parent / 'reservoir-snapshot-candidate/matrix_host_supervision_v2.py'
        self.policy = self.stack.enter_context(a.modules(policy_path.parent, (policy_path.stem,),
                              {str(policy_path): a.file_hash(policy_path)}))[policy_path.stem]
        self.stack.enter_context(patch.object(self.ca, 'validate_release_supervision', return_value=None))
        self.stack.enter_context(patch.object(self.ca, 'load_policy', return_value=self.policy))
        self.artifacts = {}
        def bound(name):
            value = self.write(self.root / name, {'synthetic_dependency': name})
            self.artifacts[value['path']] = value['sha256']; return value
        source = bound('monitor-source.json'); sampler = bound('sampler.json'); bindings = bound('bindings.json')
        sampler_binding = bound('sampler-binding.json')
        source_review = self.write(self.root / 'source-review.json', {'sampler_binding': sampler_binding})
        self.artifacts[source_review['path']] = source_review['sha256']
        self.binding = {'record_path':str(self.root/'never-read-mutable-monitor.json'), 'token':'fake-run',
                        'observer_pid':99, 'started_at':'fake historical start', 'identity_start':{'fake':'identity'},
                        'source':source, 'source_review':source_review, 'sampler':sampler, 'bindings':bindings}
        self.inputs = {'artifacts':{'/fake/source':'a'*64},
                       'admission_supervision':{name:bound(name+'.json') for name in ('helper','review','bindings')}}
        self.inputs_ref = self.write(self.root/'inputs.json',self.inputs)
        self.release = {'expires_wall':1000,'supervision':self.binding}
        self.release_ref = self.write(self.root/'release.json',self.release)
        self.seed_ref = self.write(self.run/'seed/seed.json',{'fake':'seed'})
        self.base = {'schema':'reservoir_matrix_host_supervision_v2',
                     'swap_policy':'nonincrease_between_consecutive_samples_and_not_above_initial',
                     'token':self.binding['token'],'observer_pid':99,'started_at':self.binding['started_at'],
                     'identity_start':self.binding['identity_start'],'self_sha256':source['sha256'],
                     'source_sha256':sampler['sha256'],'production_mutations':0,'qualification_cases_started':0,
                     'initial_swap_bytes':10,'invalidated':False,'state':'running'}

    def write(self,path,value):
        path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value));return a.ref(path)

    def build(self, fault=None):
        rows=[]
        for t in range(90,256,5):
            rows.append({'t_s':t,'swap_bytes':10,'pressure_level':1,'build_test_processes':[],
                         'production_identity_unchanged':True,'readyz':{'http_status':200,'body':{
                             'ready':True,'reservoir':{'status':'connected'}}}})
        if fault=='swap-rebound': rows[3]['swap_bytes']=9; rows[4]['swap_bytes']=9.5
        def monitor(end):return {**copy.deepcopy(self.base),'samples':copy.deepcopy([x for x in rows if x['t_s']<=end])}
        final=monitor(255);final.update(state='completed',completion_reason='owned_finish_request',identity_end=self.binding['identity_start'])
        if fault=='unfinished-final':final['state']='running'
        final_ref=self.write(Path(self.binding['record_path']),final)
        result={'inputs':self.inputs_ref,'release':self.release_ref,'seed':self.seed_ref,'artifacts':{},
                'started_wall':95,'completed_wall':253,'cases':[],'child_acceptances':[],
                'final_supervision':self.write(self.run/'final-supervision.json',monitor(250))}
        independent={'continuous_host':{key:self.binding[key] for key in ('source','source_review','bindings','sampler')},'cases':[]}
        root_review={'root_child_reviews':[]}; previous=None
        for ordinal,case in enumerate(('seed',)+a.RESTORE_CASES):
            stem=f'{ordinal:02d}-{case}'; directory=self.run/case; admissions=self.run/'admissions'
            start=101+ordinal*15;end=start+2;stamp=start-.1
            private=None if ordinal==0 else self.write(directory/'request.json',{'fake_request':case})
            argv=['/fake/python','-B','/fake/reader.py',case]
            waiting={'schema':'reservoir_restore_child_request_v1','run_id':'fake-run','ordinal':ordinal,'case':case,
                     'argv':argv,'directory':str(directory),'request':private,'inputs':self.inputs_ref,
                     'run_release':self.release_ref,'artifacts':{},'input_artifacts':self.inputs['artifacts'],
                     'supervision':self.binding,'previous_acceptance':previous,'production_qualified':False}
            if fault=='wrong-previous' and ordinal==3:waiting['previous_acceptance']=None
            if fault=='wrong-argv' and ordinal==3:waiting['argv']=['/unreviewed/python']
            waiting_ref=self.write(admissions/(stem+'-request.json'),waiting)
            snapshot=monitor(start-1)
            if fault=='rewritten-prefix' and ordinal==3:snapshot['samples'][0]['swap_bytes']=9
            if fault=='stale-launch' and ordinal==3:snapshot=monitor(start-20)
            snapshot_ref=self.write(admissions/(stem+'-root-supervisor.json'),snapshot)
            permit={'schema':'reservoir_restore_child_permit_v1','accepted':True,'reviewer':'root',
                    'request':waiting_ref,'run_release':self.release_ref,'previous_acceptance':previous,
                    'expires_wall':start+5,'supervisor_snapshot':snapshot_ref}
            if fault=='expired-permit' and ordinal==3:permit['expires_wall']=start-.01
            permit_ref=self.write(admissions/(stem+'-permit.json'),permit)
            observed={'path':self.binding['record_path'],'sha256':snapshot_ref['sha256']}
            if fault=='unresolved-digest' and ordinal==3:observed['sha256']='a'*64
            claim={'schema':'reservoir_restore_child_launch_claim_v1','request':waiting_ref,'permit':permit_ref,
                   'run_release':self.release_ref,'supervisor_observed':observed,'claimed_wall':stamp,
                   'automatic_retry_permitted':False,'production_qualified':False}
            if fault=='late-claim' and ordinal==3:claim['claimed_wall']=start+1
            claim_ref=self.write(admissions/(stem+'-launch-claim.json'),claim)
            child={'pid':200+ordinal,'argv':argv,'started_wall':start,'completed_wall':end,'reaped':True,
                   'returncode':0 if ordinal<2 else 1,'admission_claim':claim_ref}
            if fault=='wrong-child-claim' and ordinal==3:child['admission_claim']=permit_ref
            if ordinal==0:
                result['seed_child']=child;expected={'case':'seed','passed':True,'seed':self.seed_ref,'child':child}
            else:
                expected={'case':case,'passed':True,'child':child,'request':private};result['cases'].append(expected)
            child_ref=self.write(directory/'child-exit.json',child)
            result_ref=self.write(directory/'expected-result.json',expected)
            coverage=monitor(end+5)
            if fault=='uncovered-end' and ordinal==3:coverage=monitor(start)
            coverage_ref=self.write(admissions/(stem+'-coverage.json'),coverage)
            accepted={'schema':'reservoir_restore_child_acceptance_v1','passed':True,'production_qualified':False,
                      'run_release':self.release_ref,'request':waiting_ref,'permit':permit_ref,'claim':claim_ref,
                      'child_exit':child_ref,'result':result_ref,'coverage':coverage_ref,
                      'raw_dependencies':self.ca.result_dependencies(expected)}
            accepted_ref=self.write(admissions/(stem+'-accepted.json'),accepted)
            result['child_acceptances'].append(accepted_ref)
            review_ref=self.write(admissions/(stem+'-root-review.json'),{
                'schema':'reservoir_restore_child_root_review_v1','passed':True,'production_qualified':False,
                'case':case,'ordinal':ordinal,'child_absent':True,'child_exit':child_ref,'coverage':coverage_ref,
                'result':result_ref,'run_release':self.release_ref,'acceptance':accepted_ref})
            independent['cases'].append({'case':case,'ordinal':ordinal,'acceptance':accepted_ref,'launch_claim':claim_ref,
                'permit':permit_ref,'coverage':coverage_ref,'result':result_ref,'root_review':review_ref,'passed':True,
                'pid':child['pid'],'started_wall':start,'completed_wall':end,'returncode':child['returncode']})
            root_review['root_child_reviews'].append(review_ref); previous=accepted_ref
        self.result=result;self.spec={'restore_root_acceptance':self.write(self.root/'accepted.json',{'fake':'accepted'})}
        self.accepted={'final_supervision':final_ref}
        self.root_review=root_review;self.independent=independent
        if fault=='missing-child':result['child_acceptances'].pop()
        if fault=='reordered-child':result['child_acceptances'].reverse()
        if fault=='wrong-independent':independent['cases'][3]['pid']=9999
        return result

    def validate(self):
        with patch.object(r,'validate_reviews',return_value=(self.accepted,self.root_review,self.independent)):
            return r.validate_chain(a,self.spec,self.result,self.inputs,self.release,self.run,
                                    {'child_admission':self.ca},self.artifacts)

    def test_complete_historical_ten_chain_uses_no_mutable_monitor(self):
        self.build();recorded_final=Path(self.binding['record_path']).read_bytes()
        accepted=self.validate();self.assertEqual(accepted['child_count'],10)
        self.assertEqual(set(accepted['claims']),{'seed',*a.RESTORE_CASES})
        self.assertEqual(Path(self.binding['record_path']).read_bytes(),recorded_final)

    def test_each_authority_history_and_timing_boundary_refuses(self):
        for fault in ('wrong-previous','wrong-argv','rewritten-prefix','stale-launch','expired-permit',
                      'unresolved-digest','late-claim','wrong-child-claim','uncovered-end','missing-child',
                      'reordered-child','wrong-independent','unfinished-final','swap-rebound'):
            with self.subTest(fault=fault):
                self.build(fault)
                with self.assertRaises((ValueError,m.Refusal)):self.validate()

    def test_immutable_snapshot_tamper_cannot_be_replaced_by_current_monitor(self):
        self.build();p=self.run/'admissions/03-malformed-sidecar-root-supervisor.json'
        old=json.loads(p.read_text())
        p.write_text('{}')
        with self.assertRaises((ValueError,m.Refusal)):self.validate()

    def test_reader_dependency_change_fails_before_loading_policy(self):
        self.build();Path(self.binding['source']['path']).write_text('{}')
        with self.assertRaisesRegex(m.Refusal,'dependency'):self.validate()


class RootAcceptanceTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)
        self.qualification=self.write('q.json',{});self.release=self.write('release.json',{});self.final=self.write('final.json',{})
        self.other=self.write('other.json',{})
        self.independent={'schema':'reservoir_restore_qualification_independent_numerical_review_v1',
            'state':'accepted_completed_synthetic_cpu_matrix','passed':True,'production_qualified':False,'mlx_qualified':False,
            'reviewer':'independent','qualification':self.qualification,'release':self.release,'case_count':9,
            'seed_plus_child_count':10,'continuous_host':{'record':self.final},
            **{key:self.other for key in ('source_review','partial_positive_review','root_preclose','root_launch')}}
        self.root_review={'schema':'reservoir_restore_numeric_root_review_v1','passed':True,'production_qualified':False,
            'qualification':self.qualification,'release':self.release,'final_supervision':self.final,'host_admission':self.other}
        self.acceptance={'schema':'reservoir_restore_root_numeric_acceptance_v1','accepted':True,'reviewer':'root',
            'scope':'synthetic_cpu_fixture_only','production_qualified':False,'live_persistence_qualified':False,
            'mlx_upgrade_qualified':False,'seed_plus_nine_cases_accepted':True,'all_children_absent':True,
            'qualification':self.qualification,'final_supervision':self.final,'host_admission':self.other}
    def write(self,name,value):
        path=self.root/name;path.write_text(json.dumps(value));return a.ref(path)
    def validate(self):
        self.acceptance['root_review']=self.write('root.json',self.root_review)
        self.acceptance['independent_review']=self.write('independent.json',self.independent)
        spec={'restore_qualification':self.qualification,'restore_root_acceptance':self.write('accepted.json',self.acceptance)}
        return r.validate_reviews(a,spec,{'release':self.release})
    def test_complete_two_reviews(self):self.validate()
    def test_summary_or_different_final_refuses(self):
        for key,value in [('schema','boolean_wrapper'),('case_count',8),('production_qualified',True),
                          ('continuous_host',{'record':self.other})]:
            old=self.independent[key];self.independent[key]=value
            with self.subTest(key=key),self.assertRaises(m.Refusal):self.validate()
            self.independent[key]=old


if __name__=='__main__':unittest.main(verbosity=2)
