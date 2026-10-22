"""Synthetic service-history evidence only; no service/package/runtime invocation."""
from contextlib import ExitStack
import copy
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import production_admission as a
import reservoir_maintenance as m


class ServiceHistoryTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name).resolve();self.stack=ExitStack();self.addCleanup(self.stack.close)
        real=a.RESTORE_HARNESS
        self.ca=self.stack.enter_context(a.modules(real,('child_admission',),
                     {str(real/'child_admission.py'):a.file_hash(real/'child_admission.py')}))['child_admission']
        policy=real.parent.parent/'reservoir-snapshot-candidate/matrix_host_supervision_v2.py'
        self.policy=self.stack.enter_context(a.modules(policy.parent,(policy.stem,),
                         {str(policy):a.file_hash(policy)}))[policy.stem]
        self.stack.enter_context(patch.object(a,'SERVICE_HARNESS',self.root/'service'))
        self.stack.enter_context(patch.object(a,'modules'))
        self.source=self.write('source.json',{});self.source_review=self.write('source-review.json',{})
        self.other=self.write('other.json',{});self.manifest=self.write('manifest.json',{})
        self.binding={'record_path':str(self.root/'never-read-mutable.json'),'token':'test','observer_pid':99,
            'started_at':'historical','identity_start':{'fake':'identity'},'source':self.source,
            'source_review':self.source_review,'sampler':self.other}
        self.base={'schema':'reservoir_matrix_host_supervision_v2',
            'swap_policy':'nonincrease_between_consecutive_samples_and_not_above_initial',
            'token':'test','observer_pid':99,'started_at':'historical','identity_start':{'fake':'identity'},
            'self_sha256':self.source['sha256'],'source_sha256':self.other['sha256'],
            'production_mutations':0,'qualification_cases_started':0,'initial_swap_bytes':10,
            'state':'running','invalidated':False}
        self.q=SimpleNamespace(validate_host_gate=lambda *args,**kwargs:None)

    def write(self,name,value):
        path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value));return a.ref(path)

    def build(self,fault=None):
        rows=[{'t_s':t,'swap_bytes':10,'pressure_level':1,'production_identity_unchanged':True,
               'build_test_processes':[],'readyz':{'http_status':200,'body':{'ready':True,'reservoir':{'status':'connected'}}}}
              for t in range(90,406,5)]
        if fault=='swap-rebound':rows[4]['swap_bytes']=8;rows[5]['swap_bytes']=9
        def monitor(end):return {**copy.deepcopy(self.base),'samples':copy.deepcopy([row for row in rows if row['t_s']<=end])}
        self.final=monitor(405)
        final_ref=self.write('final.json',self.final)
        records=[];receipt_refs=[];root_refs=[];independent_rows=[];prior=None
        for ordinal in range(1,15):
            start=101+(ordinal-1)*20;end=start+4;name=f'{ordinal:02d}-fake'
            record={'started_wall':start,'completed_wall':end,'interpreter':{'executable':'/fake/python'},
                    'manifest':self.manifest['path'],'manifest_sha256':self.manifest['sha256'],
                    'variant':'original' if ordinal%2 else 'candidate','case':'normal',
                    'client_identity_before':{'pid':1000+ordinal},'child_pid':2000+ordinal}
            receipt=self.write('service/runs/'+name+'/result.json',record);receipt_refs.append(receipt);records.append(record)
            output=str(Path(receipt['path']).parent)
            argv=['/fake/python','-I','-B',str(a.SERVICE_HARNESS/'driver.py'),'run','--manifest',self.manifest['path'],
                  '--variant',record['variant'],'--case','normal','--execute-real-cpu','--output',output]
            snapshot=monitor(start-1)
            if fault=='stale-launch' and ordinal==3:snapshot=monitor(start-20)
            snapshot_ref=self.write(name+'-launch.json',snapshot)
            release={'schema':'reservoir_private_case_root_release_v1','agent':'root','production_mutations':0,
                'case_ordinal':ordinal,'argv':argv,'variant':record['variant'],'case':'normal','output':output,
                'manifest_sha256':self.manifest['sha256'],'driver_pid':1000+ordinal,'previous_case_review':prior,
                'admission_review':self.other,'supervisor_source_review':self.source_review,
                'supervisor_source_sha256':self.source['sha256'],'issued_wall':start-.1,'expires_wall':start+5,
                'supervision_snapshot':snapshot_ref}
            if fault=='wrong-argv' and ordinal==3:release['argv']=['/other/python']
            if fault=='expired-release' and ordinal==3:release['expires_wall']=start-.1
            if fault=='wrong-previous' and ordinal==3:release['previous_case_review']=None
            if fault=='wrong-driver' and ordinal==3:release['driver_pid']=999
            release_ref=self.write(name+'-release.json',release)
            coverage=monitor(end+5)
            if fault=='uncovered-end' and ordinal==3:coverage=monitor(start)
            if fault=='rewritten-prefix' and ordinal==3:coverage['samples'][0]['swap_bytes']=9
            coverage_ref=self.write(name+'-coverage.json',coverage)
            case={'schema':'reservoir_private_case_root_review_v1','passed':True,'errors':[],
                  'production_qualified':False,'mlx_qualified':False,'result':receipt,'actual_start':start,'actual_end':end,
                  'manifest':self.manifest,'host_source':self.source,'release':release_ref,'host_snapshot':coverage_ref,
                  'owned_processes_absent':{str(record['child_pid']):True,str(record['client_identity_before']['pid']):True}}
            if fault=='missing-absence' and ordinal==3:case['owned_processes_absent']={}
            root_ref=self.write(name+'-root.json',case);root_refs.append(root_ref)
            independent_rows.append({'ordinal':ordinal,'result':receipt,'root_review':root_ref,'release':release_ref,
                'host_snapshot':coverage_ref,'actual_start':start,'actual_end':end,'child_pid':record['child_pid'],
                'driver_pid':1000+ordinal,'passed_as_expected':True})
            prior=root_ref
        review={'receipts':receipt_refs};matrix_ref=self.write('matrix.json',review)
        root={'schema':'reservoir_private_matrix_root_review_v1','passed':True,'production_qualified':False,
              'accepted_for_private_numpy_only':True,'case_count':14,'matrix_review':matrix_ref,'manifest':self.manifest,
              'host_final_raw':final_ref,'root_case_reviews':root_refs,'host_admission':self.other}
        root_ref=self.write('root-review.json',root)
        independent={'schema':'reservoir_private_matrix_independent_review_v1','passed':True,'production_qualified':False,
              'accepted_for_private_numpy_only':True,'case_count':14,'matrix_review':matrix_ref,'manifest':self.manifest,
              'root_matrix_review':root_ref,'host_coverage':{'raw':final_ref},'cases':independent_rows,
              **{key:self.other for key in ('source_review','cpu_receipt','prepared_input_review')}}
        if fault=='wrong-independent':independent['cases'][3]['child_pid']=1
        independent_ref=self.write('independent.json',independent)
        accepted={'schema':'reservoir_private_matrix_root_acceptance_v1','accepted_for_private_numpy_only':True,
            'production_qualified':False,'mlx_qualified':False,'case_count':14,'matrix':matrix_ref,'root_review':root_ref,
            'independent_review':independent_ref,'source':self.manifest,'final_monitor':final_ref}
        self.spec={'real_matrix':matrix_ref,'service_root_acceptance':self.write('accepted.json',accepted)}
        self.review,self.records=review,records

    def validate(self):
        # The real frozen policy is exercised; source/host-context validation has
        # separate reader and closure tests and does no numerical work.
        with patch.object(a,'service_monitor_context',return_value=(self.ca,self.q,self.binding,self.policy,{'raw':self.other},{})):
            return a.validate_service_history(self.spec,self.review,self.records,{})

    def test_full_ordered_service_history_accepts_without_current_time_waiver(self):
        self.build();self.assertEqual(self.validate()['root_acceptance'],self.spec['service_root_acceptance'])
        self.assertFalse(Path(self.binding['record_path']).exists())

    def test_raw_release_identity_prefix_and_coverage_failures_refuse(self):
        for fault in ('wrong-argv','expired-release','wrong-previous','wrong-driver','uncovered-end',
                      'rewritten-prefix','missing-absence','wrong-independent','swap-rebound','stale-launch'):
            with self.subTest(fault=fault):
                self.build(fault)
                with self.assertRaises((m.Refusal,ValueError)):self.validate()

    def test_legacy_boolean_acceptance_refused(self):
        self.build();self.spec['service_root_acceptance']=self.write('boolean.json',{'passed':True})
        with self.assertRaisesRegex(m.Refusal,'root acceptance'):self.validate()


if __name__=='__main__':unittest.main(verbosity=2)
