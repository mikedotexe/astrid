"""Pure versioned fixture05 evidence grammar tests; never imports fixture implementations."""
from pathlib import Path
import copy
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace

import production_fixture_admission as a
import production_fixture_history as h
import production_fixture_native as n
import production_fixture_readiness as r


class Grammar(unittest.TestCase):
    def setUp(self):
        self.root=Path('/private/owned-fixture')
        self.release={'fixture':{'root':str(self.root),'token':'a'*32},'host_binding':{'uid':501},'python':'/fixed/python'}
        self.target='gui/501/com.codex.reservoir-maintenance-fixture.'+'a'*32+'.provider'

    def test_readiness_is_exactly_two_sealed_reconciliation_steps(self):
        steps=a.schedule()[0];selected=[x for x in steps if r.selected(x)]
        self.assertEqual(len(steps),71)
        self.assertEqual([x['action_id'] for x in selected],['held_verified:provider','held_verified:reservoir'])
        for step in selected:
            changed=dict(step,mode='execute');self.assertFalse(r.selected(changed))
        self.assertFalse(r.selected({'kind':'cleanup','mode':'reconcile','action_id':'held_verified:provider'}))

    def test_five_parent_sessions_are_independently_required(self):
        ready=[s for s in a.schedule()[0] if r.selected(s)]
        names=['executor:'+s['id'] for s in ready]+['cleanup_initialize','pin_initialize','finalize']
        rows=[]
        for i,name in enumerate(names):
            claim={'type':'claim','action':name,'wall_time_ns':i*10+1}
            rows.extend([claim,{'type':'result','action':name,'wall_time_ns':i*10+9,'claim_sha256':a.digest(claim)}])
        self.assertEqual(list(h.parent_intervals(rows)),names)
        with self.assertRaisesRegex(ValueError,'missing'):h.parent_intervals(rows[2:])
        with self.assertRaisesRegex(ValueError,'missing'):h.parent_intervals(rows+rows[:2])

    def test_native_fixed_queries_and_absence_classification(self):
        argv=['/bin/ps','-p','42','-o','lstart='];self.assertEqual(n.grammar(argv,self.release),('query',None))
        n.query_exit(argv,{'returncode':1,'stdout':'','stderr':''})
        for row in ({'returncode':1,'stdout':'','stderr':'denied'},{'returncode':2,'stdout':'','stderr':''}):
            with self.assertRaises(ValueError):n.query_exit(argv,row)
        with self.assertRaises(ValueError):n.grammar(['/bin/ps','aux'],self.release)
        with self.assertRaises(ValueError):n.grammar(['/usr/sbin/lsof','-nP','-iTCP:7881','-sTCP:LISTEN','-Fp'],self.release)

    def test_native_debug_has_exact_generated_guard_and_no_sudo_query(self):
        argv=['/usr/bin/sudo','--','/bin/launchctl','debug',self.target,'--program','/fixed/python','--',
            str(self.root/'model_environment.py'),'launch','--control-root',str(self.root/'hold-provider'),'--hold-only','a'*32]
        self.assertEqual(n.grammar(argv,self.release),('debug','provider'))
        for index,value in ((4,'gui/501/com.astrid.server'),(6,'/foreign/python'),(-1,'different')):
            changed=list(argv);changed[index]=value
            with self.assertRaises(ValueError):n.grammar(changed,self.release)
        with self.assertRaises(ValueError):n.grammar(['/usr/bin/sudo','--','/bin/launchctl','print',self.target],self.release)

    def test_native_query_link_cannot_cross_caller_time_or_result(self):
        link={'path':'/private/log','command_id':1,'intent_sha256':'a'*64,'created_sha256':'b'*64,'exit_sha256':'c'*64}
        row={'argv':['/bin/ps','-p','42','-o','lstart='],'returncode':1,'stdout':'','stderr':'','observed_wall_time_ns':15,'native_command':link}
        command={'link':link,'caller':'parent','intent':{'wall_time_ns':10},'exit':{k:row[k] for k in ('argv','returncode','stdout','stderr')}}
        command['exit']['wall_time_ns']=14;commands={('/private/log',1):command}
        self.assertIs(n.resolve_query(row,commands,caller='parent',earliest=10,latest=15),command)
        for kwargs in ({'caller':'child'},{'earliest':11},{'latest':14}):
            with self.subTest(kwargs=kwargs),self.assertRaises(ValueError):n.resolve_query(row,commands,**kwargs)
        changed=copy.deepcopy(row);changed['native_command']['exit_sha256']='f'*64
        with self.assertRaises(ValueError):n.resolve_query(changed,commands)
        changed=copy.deepcopy(row);changed['stdout']='foreign'
        with self.assertRaises(ValueError):n.resolve_query(changed,commands)

    def test_legacy_fixture_directory_is_not_new_consumer(self):
        self.assertEqual(a.FIXTURE.name,'owned-recovery-fixture-05')
        self.assertEqual(a.HERE.name,'production-candidate-07')


class ReadinessRows(unittest.TestCase):
    def test_readiness_latest_metadata_and_empty_listener_stderr(self):
        old={'pid':42,'started_at':'old','argv':['/python','old.py']}
        other={'pid':43,'started_at':'other','argv':['/python','other.py']}
        holder={'pid':44,'started_at':'held','argv':['/python','hold.py']}
        plan={'roles':{'provider':{'identity':old},'reservoir':{'identity':other}}}
        release={'fixture':{'targets':{'provider':'gui/501/p','reservoir':'gui/501/r'}}}
        files=SimpleNamespace(json=lambda path:{'anchors':{'provider':{'sentinel_port':20001},'reservoir':{'sentinel_port':20002}}})
        def sample(draining=False):
            rows=[]
            def row(argv,stdout='',rc=0):
                rows.append({'argv':argv,'returncode':rc,'stdout':stdout,'stderr':'','observed_wall_time_ns':10})
            def proc(value):
                pid=str(value['pid']);row(['/bin/ps','-p',pid,'-o','lstart='],value['started_at'])
                row(['/bin/ps','-ww','-p',pid,'-o','command='],' '.join(value['argv']))
                row(['/bin/ps','-p',pid,'-o','lstart='],value['started_at'])
            target=old if draining else holder
            row(['/bin/launchctl','print','gui/501/p'],'pid = '+str(target['pid']))
            proc(target);row(['/bin/ps','-p','42','-o','lstart='],rc=1)
            row(['/usr/sbin/lsof','-nP','-iTCP:20001','-sTCP:LISTEN','-Fp'],rc=1)
            row(['/usr/sbin/lsof','-nP','-a','-p',str(target['pid']),'-iTCP','-sTCP:LISTEN','-Fpn'],rc=1)
            row(['/bin/launchctl','print','gui/501/r'],'pid = 43');proc(other)
            row(['/usr/sbin/lsof','-nP','-iTCP:20002','-sTCP:LISTEN','-Fp'],'p43')
            row(['/usr/sbin/lsof','-nP','-a','-p','43','-iTCP','-sTCP:LISTEN','-Fpn'],'p43\nn127.0.0.1:20002')
            return {'queries':rows,'started_monotonic':1,'completed_monotonic':3,'observed_wall_time_ns':10,
                'target':{'process':target,'job_loaded':True,'listener_pids':[]},
                'other_role':{'process':other,'job_loaded':True,'listener_pids':[43]},'old_pid_observation':None}
        def check(value):
            # This isolates raw reconstruction; native link verification has its
            # own tests and the producer/consumer contract exercises both.
            with patch.object(r,'resolve_query',return_value={'intent':{'monotonic':1},'exit':{'monotonic':2}}):
                r.raw_sample(files,{'root':'/private/fake'},release,plan,'provider',value,{},0)
        check(sample());check(sample(draining=True))
        changed=sample();changed['queries'].append({'argv':['/bin/ps','-p','44','-o','lstart='],
            'returncode':1,'stdout':'','stderr':'','observed_wall_time_ns':10})
        with self.assertRaisesRegex(ValueError,'raw process identity'):check(changed)
        for row_index in (5,6):
            changed=sample();changed['queries'][row_index]['stderr']='inspection warning'
            with self.subTest(row=row_index),self.assertRaisesRegex(ValueError,'inspection failed'):check(changed)
