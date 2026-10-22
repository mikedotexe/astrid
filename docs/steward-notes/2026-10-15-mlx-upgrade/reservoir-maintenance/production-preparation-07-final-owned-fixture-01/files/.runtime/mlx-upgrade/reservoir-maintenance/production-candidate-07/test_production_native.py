"""Inert native boundary fakes. No actual children, signals or process queries."""
from contextlib import ExitStack
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock,patch

import production_native as n
import production_native_attempt as a
import production_preflight as p


class NativeTests(unittest.TestCase):
    def setUp(self):
        self.stack=ExitStack();self.addCleanup(self.stack.close)
        self.root=Path(self.stack.enter_context(tempfile.TemporaryDirectory())).resolve();self.root.chmod(0o700)
        self.owner={'pid':os.getpid(),'uid':os.getuid()};n.m.write_new(self.root/'owner.json',self.owner)
        self.rec=n.Recorder(self.root,self.owner)
        self.child=Mock(pid=12345);self.child.wait.return_value=0;self.child.poll.return_value=None
        self.spawn=self.stack.enter_context(patch.object(n.subprocess,'Popen',return_value=self.child))
        self.stack.enter_context(patch.object(n.os,'kill',side_effect=AssertionError('signals forbidden')))

    def test_captured_command_is_reaped_once_before_success(self):
        result=self.rec.run(['/bin/ps','-o','lstart=','-p','123'],kind='query')
        self.assertEqual(result.returncode,0);self.child.wait.assert_called_once_with(timeout=20)
        self.child.kill.assert_not_called();self.child.terminate.assert_not_called()
        record=json.loads(Path(result.native_command['path']).read_text())
        self.assertTrue(record['naturally_reaped']);self.assertEqual(record['signals_sent'],0)
        self.assertEqual(record['pid'],12345)

    def test_terminal_keeps_inherited_streams_and_existing_deadline(self):
        self.rec.run(['/usr/bin/sudo','--','/bin/launchctl','debug','fixed'],kind='debug',terminal=True,timeout=180)
        self.assertEqual(self.spawn.call_args.kwargs,{})
        self.child.wait.assert_called_once_with(timeout=180)

    def test_timeout_retains_child_and_refuses_additional_children(self):
        self.child.wait.side_effect=subprocess.TimeoutExpired(['fake'],20)
        with self.assertRaises(subprocess.TimeoutExpired):self.rec.run(['fake'],kind='query')
        row=json.loads((self.root/'command-00001/unresolved.json').read_text())
        self.assertTrue(row['may_still_be_alive']);self.assertFalse(row['naturally_reaped'])
        with self.assertRaises(n.m.Refusal):self.rec.run(['another'],kind='query')
        self.spawn.assert_called_once();self.child.kill.assert_not_called();self.child.terminate.assert_not_called()

    def test_interrupt_retains_unknown_child_and_never_waits_again(self):
        self.child.wait.side_effect=KeyboardInterrupt
        with self.assertRaises(KeyboardInterrupt):self.rec.run(['fake'],kind='query')
        self.child.wait.assert_called_once();self.child.kill.assert_not_called()
        self.assertTrue((self.root/'command-00001/intent.json').exists())

    def test_known_reap_survives_exit_publication_failure_and_poll_error(self):
        write=self.rec.write
        def broken(path,value):
            if path.name=='exit.json':raise OSError('publication failed')
            return write(path,value)
        self.child.wait.return_value=7;self.child.poll.side_effect=AssertionError('must not forget known wait result')
        with patch.object(self.rec,'write',side_effect=broken),self.assertRaises(OSError):self.rec.run(['fake'],kind='query')
        row=json.loads((self.root/'command-00001/unresolved.json').read_text())
        self.assertTrue(row['naturally_reaped']);self.assertEqual(row['known_returncode'],7)
        self.assertFalse(row['may_still_be_alive']);self.child.poll.assert_not_called()

    def test_created_publication_failure_does_not_infer_no_child(self):
        write=self.rec.write
        def broken(path,value):
            if path.name=='created.json':raise OSError('created not published')
            return write(path,value)
        with patch.object(self.rec,'write',side_effect=broken),self.assertRaises(OSError):self.rec.run(['fake'],kind='query')
        row=json.loads((self.root/'command-00001/unresolved.json').read_text())
        self.assertTrue(row['publication_uncertain']);self.assertEqual(row['pid'],12345)
        self.child.wait.assert_not_called();self.child.kill.assert_not_called()

    def test_intent_publication_failure_blocks_reuse_before_spawn(self):
        with patch.object(self.rec,'write',side_effect=OSError('intent')),self.assertRaises(OSError):self.rec.run(['fake'],kind='query')
        with self.assertRaises(n.m.Refusal):self.rec.run(['fake'],kind='query')
        self.spawn.assert_not_called()

    def test_retained_attempt_cannot_be_reattached(self):
        self.rec.run(['fake'],kind='query')
        with self.assertRaisesRegex(n.m.Refusal,'cannot be replayed'):n.Recorder(self.root,self.owner)


class GrammarTests(unittest.TestCase):
    def test_query_dispatch_has_no_mutation_authority(self):
        io=p.ReadOnlySystem()
        for argv in [('/bin/launchctl','bootout','gui/501/x'),('/usr/bin/sudo','--','/bin/launchctl','debug','x'),('/bin/sh','-c','true')]:
            with self.subTest(argv=argv),self.assertRaisesRegex(n.m.Refusal,'non-read-only'):io.command(*argv)
        with self.assertRaisesRegex(n.m.Refusal,'explicit owned'):io.command('/usr/sbin/sysctl','-n','kern.bootsessionuuid')

    def test_effects_allow_only_exact_roles_plists_and_holder_tail(self):
        labels={'provider':'p','reservoir':'r'};plists={'provider':Path('/fixed/p.plist'),'reservoir':Path('/fixed/r.plist')}
        kwargs=dict(uid=501,role='provider',labels=labels,plists=plists,holder_argv=['/python','/hold.py','launch'])
        n.mutation_argv('bootstrap',['/bin/launchctl','bootstrap','gui/501','/fixed/p.plist'],**kwargs)
        for argv in (['/bin/launchctl','bootstrap','gui/501','/foreign.plist'],['/bin/launchctl','bootout','gui/501/r']):
            with self.assertRaises(n.m.Refusal):n.mutation_argv('bootstrap',argv,**kwargs)

    def test_attempt_name_is_explicit_before_any_directory_write(self):
        for name in (None,'','../other','a/b','A','x'*65):
            with (self.subTest(name=name),patch.object(a.Path,'mkdir',side_effect=AssertionError('no write')),
                    self.assertRaises(n.m.Refusal)):
                a.reserve(Path('/unused'),'0'*64,{},'check',name,io_class=p.ReadOnlySystem)

    def test_no_force_or_context_wait_source_contract(self):
        source=Path(n.__file__).read_text()
        self.assertNotIn('subprocess.run(',source)
        self.assertNotIn('.kill(',source);self.assertNotIn('.terminate(',source)
        self.assertNotIn('with subprocess.Popen',source)


class AttemptTests(unittest.TestCase):
    def setUp(self):
        self.stack=ExitStack();self.addCleanup(self.stack.close)
        self.root=Path(self.stack.enter_context(tempfile.TemporaryDirectory())).resolve()
        self.sources=self.root/'source';self.sources.mkdir();(self.sources/'helper.py').write_text('# inert\n')
        self.script=self.sources/'production_entrypoint.py';self.script.write_text('# inert entry\n')
        self.python=self.root/'python';self.python.write_bytes(b'inert interpreter')
        self.tail=[str(self.script),'--mode','check']
        self.stack.enter_context(patch.object(a.sys,'argv',self.tail))
        self.stack.enter_context(patch.object(a.sys,'orig_argv',[str(self.python),'-B',*self.tail]))
        self.stack.enter_context(patch.object(a.sys,'executable',str(self.python)))
        self.stack.enter_context(patch.object(a,'HERE',self.sources))
        self.stack.enter_context(patch.object(n,'__file__',str(self.sources/'production_native.py')))
        self.stack.enter_context(patch.object(a,'OBSERVATIONS',self.root/'observations'))
        self.spec={'plan':{'host_binding':{'uid':os.getuid(),'boot_uuid':'boot'}},'immutable_files':{
            str(path):hashlib.sha256(path.read_bytes()).hexdigest() for path in self.sources.glob('*.py')}}
        self.spec['immutable_files'][str(self.python)]=hashlib.sha256(self.python.read_bytes()).hexdigest()
        self.path=self.root/'preparation.json';self.path.write_text(json.dumps(self.spec))
        self.sha=hashlib.sha256(self.path.read_bytes()).hexdigest()
        self.stack.enter_context(patch.object(p,'load_hold',return_value=SimpleNamespace(command_matches=lambda left,right:left==right,interpreter_paths=lambda launcher:{self.python} if launcher==self.python else set())))
        self.instances=[];outer=self
        class IO:
            def __init__(self,recorder):self.recorder=recorder;outer.instances.append(self)
            def process(self,pid):return {'pid':pid,'started_at':'fake-start','argv':self.recorder.owner['expected_argv']}
            def host(self):return outer.spec['plan']['host_binding']
        self.IO=IO

    def test_reserved_owner_is_unverified_until_exact_identity_queries_complete(self):
        io=a.reserve(self.path,self.sha,self.spec,'check','attempt-01',io_class=self.IO)
        owner=json.loads((self.root/'observations/attempt-01/owner.json').read_text())
        verified=json.loads(Path(io.native_verification['path']).read_text())
        self.assertFalse(owner['identity_verified']);self.assertEqual(verified['identity']['pid'],os.getpid())
        self.assertFalse(verified['control_authority']);self.assertEqual(owner['source_artifacts'],{k:v for k,v in self.spec['immutable_files'].items() if Path(k).parent==self.sources})
        with self.assertRaises(FileExistsError):a.reserve(self.path,self.sha,self.spec,'check','attempt-01',io_class=self.IO)
        self.assertEqual(len(self.instances),1)

    def test_failed_identity_verification_retains_attempt_without_mutation_authority(self):
        with patch.object(self.IO,'host',return_value={'boot_uuid':'foreign'}),self.assertRaises(n.m.Refusal):
            a.reserve(self.path,self.sha,self.spec,'inspect','attempt-02',io_class=self.IO)
        base=self.root/'observations/attempt-02'
        self.assertTrue((base/'owner.json').exists());self.assertFalse((base/'verified.json').exists())

    def test_original_interpreter_options_are_preserved_exactly(self):
        for ordinal,options in enumerate((['-B'],['-I','-B'])):
            original=[str(self.python),*options,*self.tail]
            with patch.object(a.sys,'orig_argv',original):
                io=a.reserve(self.path,self.sha,self.spec,'check','options-'+str(ordinal),io_class=self.IO)
            self.assertEqual(io.recorder.owner['expected_argv'],original)
            self.assertEqual(json.loads(Path(io.native_verification['path']).read_text())['identity']['argv'],original)

    def test_original_wrong_script_tail_or_family_refuses_before_queries(self):
        for original in ([str(self.python),'-B',str(self.script),'--mode','foreign'],
                         ['/foreign/python','-B',*self.tail],
                         [str(self.python),'-c','bad',*self.tail]):
            with self.subTest(original=original),patch.object(a.sys,'orig_argv',original),self.assertRaises(n.m.Refusal):
                a.reserve(self.path,self.sha,self.spec,'check','wrong-invocation',io_class=self.IO)
        self.assertFalse((self.root/'observations').exists());self.assertEqual(self.instances,[])

    def test_mutating_sink_requires_verified_executor_and_current_session(self):
        import transaction_execution
        host=SimpleNamespace(root=self.root,plan={},io=SimpleNamespace(recorder=None))
        with patch.object(transaction_execution,'require_active',return_value=SimpleNamespace(begin={})),self.assertRaises(n.m.Refusal):
            a.mutation_recorder(host)
        self.assertFalse((self.root/'native-commands').exists())

    def test_foreign_loaded_native_alias_refuses_before_any_directory(self):
        with patch.object(n,'__file__','/foreign/native.py'),self.assertRaisesRegex(n.m.Refusal,'origin'):
            a.reserve(self.path,self.sha,self.spec,'check','attempt-03',io_class=self.IO)
        self.assertFalse((self.root/'observations').exists())


class RetainedProofTests(unittest.TestCase):
    def test_host_completion_requires_native_exit_reference(self):
        with self.assertRaisesRegex(n.m.Refusal,'reference missing'):
            n.host_links(Path('/owned'),{'host_binding':{'uid':501}},[{'operation':'debug_result','role':'provider','returncode':0}],Mock())

    def test_unresolved_native_attempt_is_never_cleared_by_a_holder(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder).resolve();attempt=root/'native-commands/a';command=attempt/'command-00001';command.mkdir(parents=True)
            plan={'transaction_id':'a'*32};session={'session_id':'b'*32,'plan_sha256':n.m.digest(plan),'root':str(root),'executor_identity':{'pid':42}}
            values={attempt/'owner.json':{'schema':'production_native_effect_owner_v1','root':str(attempt),'transaction_id':'a'*32,
                'plan_sha256':n.m.digest(plan),'pid':42,'executor_session':session},
                root/'execution-sessions'/('b'*32+'-begin.json'):session}
            owner=values[attempt/'owner.json']
            values[command/'intent.json']={'owner':owner,'command_id':1};values[command/'unresolved.json']={'pid':99,'may_still_be_alive':True}
            for path,value in values.items():path.parent.mkdir(parents=True,exist_ok=True);path.write_text(json.dumps(value))
            evidence=SimpleNamespace(read=lambda path,record=False: values[Path(path)] if record else hashlib.sha256(Path(path).read_bytes()).hexdigest())
            result=n.retained_commands(root,plan,evidence)
            self.assertEqual(len(result),1);self.assertIn('no repeat authorized',result[0])


class DecodeFailureTests(NativeTests):
    # Only the new boundary is declared; do not duplicate inherited test cases.
    def test_invalid_output_preserves_observed_natural_reap(self):
        def fake_spawn(argv,**kwargs):
            kwargs['stdout'].write(b'\xff');kwargs['stdout'].flush();return self.child
        self.spawn.side_effect=fake_spawn;self.child.poll.side_effect=AssertionError('known reap must persist')
        with self.assertRaises(UnicodeDecodeError):self.rec.run(['fake'],kind='query')
        row=json.loads((self.root/'command-00001/unresolved.json').read_text())
        self.assertTrue(row['naturally_reaped']);self.assertEqual(row['known_returncode'],0)
        self.assertFalse(row['may_still_be_alive']);self.child.poll.assert_not_called()

# Reuse setup helpers, not the base test inventory a second time.
for _name in tuple(vars(NativeTests)):
    if _name.startswith('test_'):setattr(DecodeFailureTests,_name,None)
