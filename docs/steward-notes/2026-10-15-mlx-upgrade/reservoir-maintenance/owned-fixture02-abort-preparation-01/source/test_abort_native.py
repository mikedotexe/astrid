"""Mock-only command boundary tests and private scratch filesystem guards."""
from __future__ import annotations
import ast
import copy
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import Mock,patch

import abort_evidence as e
import abort_native as n


def make_host():
    t={'run_root':'/owned/run','abort_root':'/owned/abort','python':'/pinned/bin/python3.14',
       'python_files':{'/pinned/bin/python3.14':'a','/pinned/Python':'b'},
       'workers':{'provider':{'pid':9267,'started_at':'old','argv':['/pinned/Python','worker']},
                  'reservoir':{'pid':9217,'started_at':'old','argv':['/pinned/Python','worker']}},
       'provider_holder':{'pid':10721,'started_at':'held','argv':[]},
       'ports':{'provider':50551,'reservoir':50552},
       'targets':{'provider':'gui/501/owned.provider','reservoir':'gui/501/owned.reservoir','pin':'gui/501/owned.pin'},
       'provider_guard':{'transaction_id':'old','actor':'owned'},'reservoir_guard':{'transaction_id':'new','actor':'abort'}}
    host=n.NativeHost(t,{},{});host.allow=Mock();host.process=Mock(return_value={'pid':123,'started_at':'now','argv':['command']})
    return host


class NativeTests(unittest.TestCase):
    def setUp(self):self.host=make_host();self.rows=[]
    def record(self,kind,**values):self.rows.append({'type':kind,**values})
    def child(self,*,error=None,code=0):
        child=Mock();child.pid=123;child.returncode=None
        if error is not None:child.wait.side_effect=error
        else:child.wait.return_value=code
        return child
    def effect(self,child,precondition=lambda:None):
        with patch.object(n.subprocess,'Popen',return_value=child):
            return self.host.effect('debug_reservoir',['/usr/bin/sudo','--','/bin/launchctl'],self.record,180,precondition)
    def test_action_timeout_keeps_child_without_force(self):
        child=self.child(error=subprocess.TimeoutExpired('sudo',180))
        with self.assertRaises(subprocess.TimeoutExpired):self.effect(child)
        child.kill.assert_not_called();child.terminate.assert_not_called()
        self.assertEqual(self.rows[-1]['type'],'command_unresolved');self.assertTrue(self.rows[-1]['may_still_be_alive'])
        self.assertEqual(self.rows[-1]['pid'],123);self.assertEqual(self.host.created_children,[child])
    def test_action_interruption_keeps_child_without_force(self):
        child=self.child(error=KeyboardInterrupt())
        with self.assertRaises(KeyboardInterrupt):self.effect(child)
        child.kill.assert_not_called();child.terminate.assert_not_called();self.assertFalse(self.rows[-1]['naturally_reaped'])
    def test_nonzero_return_preserves_known_reap_not_live_claim(self):
        child=self.child(code=1)
        with self.assertRaisesRegex(ValueError,'nonzero'):self.effect(child)
        self.assertEqual(self.rows[-1]['type'],'command_exit');self.assertTrue(self.rows[-1]['naturally_reaped'])
        self.assertFalse(any(r['type']=='command_unresolved' for r in self.rows))
    def test_exit_record_failure_keeps_known_reap(self):
        child=self.child()
        def record(kind,**values):
            if kind=='command_exit':raise OSError('disk full')
            self.record(kind,**values)
        with patch.object(n.subprocess,'Popen',return_value=child),self.assertRaisesRegex(OSError,'disk full'):
            self.host.effect('debug',[],record,1,lambda:None)
        self.assertTrue(self.rows[-1]['naturally_reaped']);self.assertFalse(self.rows[-1]['may_still_be_alive'])
        self.assertEqual(self.rows[-1]['known_returncode'],0)
    def test_final_precondition_runs_after_allow_and_before_popen(self):
        order=[];self.host.allow.side_effect=lambda:order.append('allow')
        child=self.child()
        with patch.object(n.subprocess,'Popen',side_effect=lambda argv:order.append('popen') or child):
            self.host.effect('test',[],self.record,1,lambda:order.append('final'))
        self.assertEqual(order,['allow','final','popen'])
    def test_final_precondition_refusal_never_starts_action(self):
        with patch.object(n.subprocess,'Popen') as popen,self.assertRaisesRegex(ValueError,'drift'):
            self.host.effect('test',[],self.record,1,lambda:e.require(False,'drift'))
        popen.assert_not_called();self.assertIsNone(self.rows[-1]['pid'])
    def test_query_timeout_retains_child_without_force(self):
        child=self.child();child.communicate.side_effect=subprocess.TimeoutExpired('ps',5)
        with patch.object(n.subprocess,'Popen',return_value=child),self.assertRaisesRegex(RuntimeError,'no kill or retry'):
            self.host.query(['/bin/ps','-p','123','-o','lstart='])
        child.kill.assert_not_called();child.terminate.assert_not_called();self.assertTrue(self.host.queries[-1]['may_still_be_alive'])
    def test_query_interruption_retains_child_without_force(self):
        child=self.child();child.communicate.side_effect=KeyboardInterrupt()
        with patch.object(n.subprocess,'Popen',return_value=child),self.assertRaisesRegex(RuntimeError,'no kill or retry'):
            self.host.query(['/bin/ps','-p','123','-o','lstart='])
        child.kill.assert_not_called();child.terminate.assert_not_called();self.assertEqual(self.host.queries[-1]['error_type'],'KeyboardInterrupt')
    def test_action_cannot_enter_query_path(self):
        with patch.object(n.subprocess,'Popen') as popen,self.assertRaisesRegex(ValueError,'read-only'):
            self.host.query(['/bin/launchctl','bootout','anything'])
        popen.assert_not_called()
    def test_signal_requires_durable_debug_success(self):
        self.host.ready_reservoir=Mock()
        with tempfile.TemporaryDirectory() as tmp:
            self.host.abort=Path(tmp).resolve()
            with patch.object(n.os,'kill') as kill,self.assertRaisesRegex(ValueError,'durable successful debug'):
                self.host.signal_reservoir(self.record)
        kill.assert_not_called()
    def test_changed_guard_after_debug_prevents_signal(self):
        self.host.ready_reservoir=Mock();self.host.ready_provider_holder=Mock();success={'returncode':0,'naturally_reaped':True}
        self.host.debug_confirmation={'result':success}
        with tempfile.TemporaryDirectory() as tmp:
            self.host.abort=Path(tmp).resolve();(self.host.abort/'event-00001.json').write_text('{}')
            def read(path):
                if path.name.startswith('event-'):return {'type':'result','action':'debug_reservoir','result':success}
                return {'transaction_id':'foreign'}
            with patch.object(e,'read',side_effect=read),patch.object(n.os,'kill') as kill,self.assertRaisesRegex(ValueError,'guard changed'):
                self.host.signal_reservoir(self.record)
        kill.assert_not_called()
    def test_signal_exact_pid_once_after_current_guard(self):
        self.host.ready_reservoir=Mock();self.host.ready_provider_holder=Mock();success={'returncode':0,'naturally_reaped':True}
        self.host.debug_confirmation={'result':success}
        with tempfile.TemporaryDirectory() as tmp:
            self.host.abort=Path(tmp).resolve();(self.host.abort/'event-00001.json').write_text('{}')
            def read(path):
                if path.name.startswith('event-'):return {'type':'result','action':'debug_reservoir','result':success}
                return self.host.guard('reservoir')
            with patch.object(e,'read',side_effect=read),patch.object(n.os,'kill') as kill:
                self.host.signal_reservoir(self.record)
                with self.assertRaisesRegex(ValueError,'already attempted'):self.host.signal_reservoir(self.record)
                kill.assert_called_once_with(9217,signal.SIGTERM)
    def test_holder_changes_during_full_revalidation_blocks_bootout(self):
        expected={'pid':100,'started_at':'held','argv':['holder']}
        good={'loaded':True,'held':True,'identity':expected,'port_pids':[]}
        bad={**good,'identity':{**expected,'started_at':'reused'}}
        self.host.inspect=Mock(side_effect=[good,bad])
        with patch.object(n.subprocess,'Popen') as popen,self.assertRaisesRegex(ValueError,'changed before bootout'):
            self.host.remove_held('reservoir',expected,self.record)
        popen.assert_not_called();self.assertIn('reservoir',self.host.removed)
    def test_stateful_worker_cannot_be_booted_out(self):
        self.host.inspect=Mock(return_value={'loaded':True,'held':False,'identity':self.host.t['workers']['reservoir'],'port_pids':[9217]})
        with patch.object(n.subprocess,'Popen') as popen,self.assertRaisesRegex(ValueError,'only exact'):
            self.host.remove_held('reservoir',self.host.t['workers']['reservoir'],self.record)
        popen.assert_not_called()
    def test_guard_release_requires_absent_job_pid_and_listener(self):
        self.host.job=Mock(return_value=(True,100))
        with self.assertRaisesRegex(ValueError,'absence required'):self.host.release_guard('reservoir',{'pid':100},self.record)
    def holder_observer(self):
        host=self.host;host.job=Mock(return_value=(True,11001));host.listeners=Mock(return_value=[]);host.port_pids=Mock(return_value=[])
        ident={'pid':11001,'started_at':'now','argv':host.hold_argv('reservoir')}
        host.process=Mock(return_value=ident)
        receipt={'schema':'model_environment_held_v1','pid':11001,'started_at':'now','hold':host.guard('reservoir'),
                 'hold_only':True,'launcher_sha256':'digest','model_imported':False,'listener_opened':False}
        def read(path):return receipt if path.parent.name=='held' else host.guard('reservoir')
        return ident,receipt,read
    def test_exact_launcher_family_waits_until_framework_and_receipt(self):
        ident,receipt,read=self.holder_observer();ident['argv'][0]=self.host.t['python']
        with patch.object(e,'read',side_effect=read),patch.object(e,'sha',return_value='digest'),patch.object(e,'reference',return_value={}),patch.object(Path,'exists',return_value=True):
            first=self.host.inspect('reservoir');self.assertFalse(first['held']);self.assertTrue(first['holder_pending'])
            ident['argv'][0]='/pinned/Python';self.assertTrue(self.host.inspect('reservoir')['held'])
    def test_pending_holder_pid_start_reuse_is_refused(self):
        ident,receipt,read=self.holder_observer()
        with patch.object(e,'read',side_effect=read),patch.object(Path,'exists',return_value=False):
            self.host.inspect('reservoir');ident['started_at']='reused'
            with self.assertRaisesRegex(ValueError,'identity changed'):self.host.inspect('reservoir')
    def test_unknown_or_parenthesized_holder_argv_is_refused(self):
        for argv in (['(python3.14)'],['foreign',*self.host.hold_argv('reservoir')[1:]]):
            with self.subTest(argv=argv):
                ident,_,_=self.holder_observer();ident['argv']=argv
                with self.assertRaisesRegex(ValueError,'foreign holder'):self.host.inspect('reservoir')
    def test_signalled_old_pid_publication_gap_is_pending_not_held(self):
        self.host.signal_used=True;self.host.job=Mock(return_value=(True,9217));self.host.process=Mock(return_value=None)
        self.host.port_pids=Mock(return_value=[])
        with patch.object(e,'read',return_value=self.host.guard('reservoir')):
            view=self.host.inspect('reservoir')
        self.assertFalse(view['held']);self.assertIsNone(view['identity']);self.assertTrue(view['loaded'])
    def test_unsignalled_stale_pid_cannot_be_treated_as_pending(self):
        self.host.job=Mock(return_value=(True,9217));self.host.process=Mock(return_value=None)
        with self.assertRaisesRegex(ValueError,'stale disappeared'):self.host.inspect('reservoir')
    def test_signalled_publication_gap_rejects_listener_or_changed_guard(self):
        self.host.signal_used=True;self.host.job=Mock(return_value=(True,9217));self.host.process=Mock(return_value=None)
        for ports,guard in [([9217],self.host.guard('reservoir')),([],{'foreign':True})]:
            with self.subTest(ports=ports,guard=guard):
                self.host.port_pids=Mock(return_value=ports)
                with patch.object(e,'read',return_value=guard),self.assertRaisesRegex(ValueError,'unproved old-worker'):
                    self.host.inspect('reservoir')
    def test_shared_lock_opens_existing_inode_without_create(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.host.root=Path(tmp).resolve();path=self.host.root/'execution.lock';path.touch(mode=0o600)
            info=path.stat();self.host.t['execution_lock_identity']={'device':info.st_dev,'inode':info.st_ino}
            real_open=os.open
            with patch.object(n.fcntl,'flock') as flock,patch.object(n.os,'open',wraps=real_open) as opened:
                with self.host.lock():self.assertIsNotNone(self.host.lock_fd)
            self.assertFalse(opened.call_args.args[1]&os.O_CREAT);flock.assert_called_once()
            self.assertIsNone(self.host.lock_fd)
    def test_saved_capture_retains_exact_bytes_and_source_hashes(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.host.root=Path(tmp).resolve()
            for name,value in [('state.json','{"fixture_astrid":{"ticks":11}}\n'),('reservoir.events.jsonl','{"phase":"drained"}\n')]:
                (self.host.root/name).write_text(value)
            captured=self.host.capture_saved()
            for name,row in captured.items():
                self.assertEqual(row['utf8'].encode(),(self.host.root/name).read_bytes())
                self.assertEqual(row['sha256'],e.sha(self.host.root/name));self.assertEqual(row['bytes'],len(row['utf8'].encode()))
    def test_lock_inode_drift_refuses_before_flock(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.host.root=Path(tmp).resolve();path=self.host.root/'execution.lock';path.touch(mode=0o600)
            self.host.t['execution_lock_identity']={'device':0,'inode':0}
            with patch.object(n.fcntl,'flock') as flock,self.assertRaisesRegex(ValueError,'foreign execution lock'):
                with self.host.lock():pass
            flock.assert_not_called()
    def test_evidence_writes_cannot_touch_original_or_existing_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp).resolve();abort=root/'abort';abort.mkdir(mode=0o700);old=root/'original';old.mkdir(mode=0o700)
            with patch.object(e,'ABORT',abort):
                with self.assertRaisesRegex(ValueError,'outside abort'):n.write_new(old/'new.json',{})
                n.write_new(abort/'one.json',{'one':1})
                with self.assertRaises(FileExistsError):n.write_new(abort/'one.json',{'two':2})
                (abort/'alias').symlink_to(old,target_is_directory=True)
                with self.assertRaisesRegex(ValueError,'outside abort'):n.write_new(abort/'alias/new.json',{})
            self.assertEqual(list(old.iterdir()),[])
    def test_no_kill_timeout_helper_or_generic_command_runner(self):
        tree=ast.parse(Path(n.__file__).read_text())
        forbidden=[]
        for node in ast.walk(tree):
            if isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute):
                if node.func.attr in ('run','terminate','kill') and not (isinstance(node.func.value,ast.Name) and node.func.value.id=='os' and node.func.attr=='kill'):
                    forbidden.append(node.func.attr)
        self.assertEqual(forbidden,[])


if __name__=='__main__':unittest.main()
