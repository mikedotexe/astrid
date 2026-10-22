"""Authored fake-only native boundaries; no real child, query or signal."""
from __future__ import annotations
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock

import owned_fixture_commands as n
import owned_fixture_records as r


class Commands(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup)
        self.root=Path(tmp.name).resolve();self.base=self.root/'parent';self.base.mkdir(mode=0o700)
        self.token='a'*32;self.uid=os.getuid();self.python='/exact/selected/python'
        self.release={'fixture':{'root':str(self.root/'fixture'),'token':self.token},'host_binding':{'uid':self.uid},
            'python':self.python,'sudo_debug':True,'artifacts':{str(n.HERE/'owned_fixture_commands.py'):r.reference(n.__file__)['sha256']}}
        self.release_ref=r.write(self.root/'release.json',self.release)
        self.owner={'pid':os.getpid(),'caller':'parent'};owner_ref=r.write(self.base/'owner.json',self.owner)
        self.command=n.Commands(self.release,self.release_ref,self.base,self.owner,owner_ref)
        self.addCleanup(os.close,self.command.fd)
        self.enterContext(mock.patch.object(n,'_enabled'))
        self.spawn=self.enterContext(mock.patch.object(n.subprocess,'Popen',side_effect=AssertionError('unmocked process')))

    def target(self,role):return f'gui/{self.uid}/com.codex.reservoir-maintenance-fixture.{self.token}.{role}'
    def rows(self):return [json.loads(line) for line in self.command.path.read_bytes().splitlines()]
    def child(self,code=0,output=''):
        child=mock.MagicMock(pid=77777,returncode=None)
        def communicate(**kwargs):child.returncode=code;return output,''
        def wait(**kwargs):child.returncode=code;return code
        child.communicate.side_effect=communicate;child.wait.side_effect=wait
        self.spawn.side_effect=None;self.spawn.return_value=child
        return child

    def test_exact_queries_include_proven_absence(self):
        queries=[['/bin/ps','-p','22','-o','lstart='],['/bin/ps','-ww','-p','22','-o','command='],
            ['/usr/sbin/lsof','-nP','-a','-p','22','-iTCP','-sTCP:LISTEN','-Fpn'],
            ['/usr/sbin/lsof','-nP','-iTCP:45000','-sTCP:LISTEN','-Fp'],
            ['/usr/sbin/sysctl','-n','kern.bootsessionuuid'],['/usr/bin/sw_vers','-productVersion'],
            ['/bin/launchctl','print',self.target('provider')]]
        for argv in queries:self.assertEqual(self.command.verify(argv,False,20),'query')
        with self.assertRaises(ValueError):self.command.verify(queries[-1],False,180)
        child=self.child(code=1);result=self.command.run(queries[0])
        self.assertEqual(result.returncode,1);self.assertTrue(self.rows()[-1]['naturally_reaped'])
        child.kill.assert_not_called();child.terminate.assert_not_called()

    def test_foreign_or_malformed_query_tails_never_spawn(self):
        for argv in ([],['/bin/ps','aux'],['/bin/ps','-p','1','-o','lstart='],
            ['/bin/ps','-p','22','-o','env='],['/usr/sbin/lsof','-nP','-iTCP:8090','-sTCP:LISTEN','-Fp'],
            ['/usr/sbin/sysctl','-w','kern.test=1'],['/usr/bin/sw_vers','-buildVersion'],
            ['/bin/launchctl'],['/bin/launchctl','print','gui/1/foreign']):
            with self.subTest(argv=argv),self.assertRaises(ValueError):self.command.run(argv)
        self.spawn.assert_not_called();self.assertEqual(self.rows(),[])

    def debug(self,role='reservoir'):
        root=Path(self.release['fixture']['root'])
        return ['/usr/bin/sudo','--','/bin/launchctl','debug',self.target(role),'--program',self.python,'--',
            str(root/'model_environment.py'),'launch','--control-root',str(root/('hold-'+role)),'--hold-only',self.token]

    def test_exact_debug_guard_program_and_role_required(self):
        self.assertEqual(self.command.verify(self.debug(),True,180),'effect')
        for index,value in ((4,self.target('pin')),(6,'/foreign/python'),(8,'/foreign/helper'),
                             (11,'/foreign/hold'),(13,self.token+'-other')):
            argv=self.debug();argv[index]=value
            with self.subTest(index=index),self.assertRaises(ValueError):self.command.verify(argv,True,180)
        with self.assertRaises(ValueError):self.command.verify(self.debug()+['extra'],True,180)
        with self.assertRaises(ValueError):self.command.verify(self.debug(),False,20)
        with self.assertRaisesRegex(ValueError,'privilege form'):self.command.verify(self.debug()[2:],False,20)
        self.spawn.assert_not_called()

    def test_natural_effect_reap_is_distinct_from_nonzero_effect_success(self):
        child=self.child(code=5);result=self.command.run(self.debug(),terminal=True,timeout=180)
        self.assertEqual(result.returncode,5);row=self.rows()[-1]
        self.assertEqual(row['returncode'],5);self.assertTrue(row['naturally_reaped'])
        self.assertIsNone(row['stdout']);self.assertEqual(row['signals_sent'],0)
        child.wait.assert_called_once_with(timeout=180);child.kill.assert_not_called()

    def test_timeout_preserves_created_child_without_force_or_second_command(self):
        child=self.child();child.communicate.side_effect=subprocess.TimeoutExpired('inert',20)
        with self.assertRaises(subprocess.TimeoutExpired):self.command.run(['/bin/ps','-p','22','-o','lstart='])
        rows=self.rows();self.assertEqual([x['type'] for x in rows],['intent','created','unresolved'])
        self.assertEqual(rows[-1]['pid'],child.pid);self.assertTrue(rows[-1]['may_still_be_alive'])
        with self.assertRaisesRegex(ValueError,'failed'):self.command.run(['/bin/ps','-p','22','-o','lstart='])
        self.spawn.assert_called_once();child.kill.assert_not_called();child.terminate.assert_not_called()

    def test_keyboard_interrupt_has_same_no_force_boundary(self):
        child=self.child();child.wait.side_effect=KeyboardInterrupt()
        with self.assertRaises(KeyboardInterrupt):self.command.run(self.debug(),terminal=True,timeout=180)
        self.assertEqual(self.rows()[-1]['error_type'],'KeyboardInterrupt')
        child.kill.assert_not_called();child.terminate.assert_not_called();child.__enter__.assert_not_called()

    def test_created_publication_failure_retains_known_pid(self):
        child=self.child();original=self.command.append
        def append(event_type,*args,**kwargs):
            if event_type=='created':raise OSError('inert publication failure')
            return original(event_type,*args,**kwargs)
        with mock.patch.object(self.command,'append',side_effect=append),self.assertRaises(OSError):
            self.command.run(['/bin/ps','-p','22','-o','lstart='])
        row=self.rows()[-1];self.assertEqual(row['pid'],child.pid);self.assertIsNone(row['created_sha256'])
        self.assertTrue(row['may_still_be_alive']);child.communicate.assert_not_called();child.kill.assert_not_called()

    def test_exit_publication_failure_retains_known_natural_reap(self):
        child=self.child();original=self.command.append
        def append(event_type,*args,**kwargs):
            if event_type=='exit':raise OSError('inert exit publication failure')
            return original(event_type,*args,**kwargs)
        with mock.patch.object(self.command,'append',side_effect=append),self.assertRaises(OSError):
            self.command.run(['/bin/ps','-p','22','-o','lstart='])
        row=self.rows()[-1];self.assertTrue(row['naturally_reaped']);self.assertFalse(row['may_still_be_alive'])
        self.assertEqual(row['known_returncode'],0);child.kill.assert_not_called()

    def test_intent_publication_failure_precedes_popen_and_latches_refusal(self):
        with mock.patch.object(self.command,'append',side_effect=OSError('inert full journal')),self.assertRaises(OSError):
            self.command.run(['/bin/ps','-p','22','-o','lstart='])
        self.assertTrue(self.command.failed);self.spawn.assert_not_called()

    def test_journal_chain_and_pid_ownership_bind_natural_result(self):
        self.child(output='inert');out=self.command.run(['/bin/ps','-p','22','-o','lstart=']);rows=self.rows()
        previous=r.digest(self.owner)
        for index,row in enumerate(rows,1):
            self.assertEqual(row['sequence'],index);self.assertEqual(row['previous_sha256'],previous)
            self.assertEqual(row['caller_owner_sha256'],r.digest(self.owner));previous=r.digest(row)
        self.assertEqual(out.native_command['exit_sha256'],r.digest(rows[-1]))
        self.assertEqual(rows[2]['created_sha256'],r.digest(rows[1]))
        self.assertEqual(rows[1]['intent_sha256'],r.digest(rows[0]))

    def test_journal_rewrite_refuses_before_popen(self):
        self.command.path.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError,'changed'):self.command.run(['/bin/ps','-p','22','-o','lstart='])
        self.spawn.assert_not_called()



class ProducerConsumerContract(unittest.TestCase):
    """Actual producer command rows and sample accepted by independent06 parser."""
    setUp=Commands.setUp
    rows=Commands.rows
    target=Commands.target

    def consumers(self):
        import importlib
        import sys
        self.enterContext(mock.patch.dict(sys.modules))
        for name in ('production_fixture_admission','production_fixture_native','production_fixture_readiness'):
            sys.modules.pop(name,None)
        with mock.patch.object(sys,'path',[str(n.HERE.parent/'production-candidate-07'),*sys.path]):
            return importlib.import_module('production_fixture_readiness')

    def sample(self):
        import shlex
        import time
        import owned_fixture_coordinator as coordinator
        import owned_fixture_system as system
        import owned_recovery_host as host_module
        fixture=Path(self.release['fixture']['root']);fixture.mkdir(mode=0o700)
        r.write(fixture/'fixture.owner.json',{'schema':'reservoir_maintenance_owned_fixture_v1',
              'token':self.token,'root':str(fixture),'uid':self.uid})
        old={'pid':101,'started_at':'old-start','argv':['old-python','worker']}
        holder={'pid':102,'started_at':'holder-start','argv':['selected-python','holder']}
        other={'pid':202,'started_at':'other-start','argv':['selected-python','other-worker']}
        identities={102:holder,202:other}
        def popen(argv,**kwargs):
            rc=0;out=''
            if argv[:2]==['/bin/launchctl','print']:
                out='pid = '+('102' if argv[2].endswith('.provider') else '202')+'\n'
            elif argv[0]=='/bin/ps':
                pid=int(argv[argv.index('-p')+1]);ident=identities.get(pid)
                if ident is None:rc=1
                elif argv[-1]=='lstart=':out=ident['started_at']+'\n'
                else:out=shlex.join(ident['argv'])+'\n'
            elif '-p' in argv:
                if argv[argv.index('-p')+1]=='202':out='p202\nn127.0.0.1:45002\n'
                else:rc=1
            elif '-iTCP:45002' in argv:out='p202\n'
            else:rc=1
            child=mock.Mock(pid=900000+self.command.command_id,returncode=None)
            def communicate(**kwargs):child.returncode=rc;return out,''
            child.communicate.side_effect=communicate;return child
        self.spawn.side_effect=popen
        self.enterContext(mock.patch.object(coordinator,'FIXTURE_EXECUTION_ENABLED',True))
        self.enterContext(mock.patch.object(n,'_context',self.command))
        adapter=system.OwnedSystem(fixture,self.token,self.python)
        guard={'transaction_id':self.token}
        host=SimpleNamespace(root=fixture,plan={'roles':{'provider':{'identity':old}}},
            core={'transaction_execution':SimpleNamespace(require_active=lambda *_:None)},
            signalled={('provider',101,'old-start')},guard=lambda:guard,
            env=SimpleNamespace(assert_hold=lambda *_:None,command_matches=lambda a,b:a==b),
            control=lambda role:fixture/('hold-'+role),io=adapter,remember_ports=lambda *_:None,
            known_ports={'provider':{45001}},ports={'provider':45001},target=self.target,
            hold_argv=lambda *_:holder['argv'])
        began=time.monotonic();low=time.time_ns();target,seen=host_module.readiness_transition(host,'provider',old,guard)
        loaded,pid=adapter.job('reservoir');ident=adapter.process(pid)
        owners=adapter.port_pids(45002);adapter.listener_ports(pid)
        other_view={'target':self.target('reservoir'),'job_loaded':loaded,'process':ident,'listener_pids':owners,'held':None}
        sample={'started_monotonic':began,'completed_monotonic':time.monotonic(),'observed_wall_time_ns':time.time_ns(),
                'target':target,'old_pid_observation':seen,'other_role':other_view,'guard':guard,'queries':adapter.observations}
        commands={};rows=self.rows()
        for i in range(0,len(rows),3):
            intent,created,end=rows[i:i+3]
            link={'path':str(self.command.path),'command_id':intent['command_id'],'intent_sha256':r.digest(intent),
                  'created_sha256':r.digest(created),'exit_sha256':r.digest(end)}
            commands[(link['path'],link['command_id'])]={'link':link,'intent':intent,'created':created,'exit':end,'caller':'parent'}
        files=SimpleNamespace(json=lambda path:{'anchors':{'provider':{'sentinel_port':45001},'reservoir':{'sentinel_port':45002}}})
        release={**self.release,'fixture':{**self.release['fixture'],'targets':{role:self.target(role) for role in ('provider','reservoir','pin')}}}
        plan={'roles':{'provider':{'identity':old},'reservoir':{'identity':other}}}
        return files,{'root':str(fixture)},release,plan,'provider',sample,commands,low

    def test_actual_producer_native_rows_and_readiness_sample_match06_reader(self):
        reader=self.consumers();args=self.sample();reader.raw_sample(*args)
        self.assertTrue(args[5]['target']['holder_pending']);self.assertIsNone(args[5]['target']['held'])
        self.assertIsNone(args[5]['old_pid_observation'])

    def test_missing_stale_or_foreign_query_cannot_replace_actual_sample(self):
        import copy
        reader=self.consumers();args=list(self.sample())
        for variant in ('missing','stale','foreign'):
            altered=copy.deepcopy(args[5])
            if variant=='missing':altered['queries']=altered['queries'][:-1]
            elif variant=='stale':altered['queries'][0]['observed_wall_time_ns']=args[7]-1
            else:altered['queries'][0]['native_command']['exit_sha256']='0'*64
            trial=args.copy();trial[5]=altered
            with self.subTest(variant=variant),self.assertRaises(ValueError):reader.raw_sample(*trial)



if __name__=='__main__':unittest.main()
