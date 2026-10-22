"""Small fake-file/semantic tests only. No actual numeric or reservoir imports."""
from __future__ import annotations
import ast
import copy
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
import restore_verifier as v


class FakeArray:
    def __init__(self,value):self.value=value
    def round(self,digits):return FakeArray([round(float(x),digits) for x in self.value])
    def tolist(self):return self.value

class FakeNp:
    float32='fake-float32'
    @staticmethod
    def asarray(value,dtype=None):return FakeArray(value)


def thermostat():
    return {'entropy_target':.1,'rho':.97,'tick':7,'last_entropy':None,'last_saturation':.01,
            'warmup_entropies':[.1,.2,.3], 'buffer_tail':[[.12345678,.2],[.3,.4],[.5,.6]]}


class SemanticsTests(unittest.TestCase):
    def test_floor_tail_and_full_fields(self):
        result=v.thermostat_expected(thermostat(),.24,FakeNp,2,tail=2)
        self.assertEqual(result,{'entropy_target':.24,'rho':.97,'tick':7,'last_entropy':None,
                                 'last_saturation':.01,'warmup_entropies':[.2,.3], 'buffer_tail':[[.3,.4],[.5,.6]]})
        original=thermostat();original['entropy_target']=None
        self.assertIsNone(v.thermostat_expected(original,.24,FakeNp,2)['entropy_target'])

    def test_thermostat_incomplete_nonfinite_shapes_rejected(self):
        changes=[lambda r:r.pop('tick'),lambda r:r.update(extra=True),lambda r:r.update(rho=float('nan')),
                 lambda r:r.update(tick=True),lambda r:r.update(buffer_tail=[[1]]),
                 lambda r:r.update(warmup_entropies=[float('inf')]),lambda r:r.update(last_entropy='nan')]
        for change in changes:
            row=thermostat();change(row)
            with self.subTest(change=change),self.assertRaises(ValueError):v.thermostat_expected(row,.24,FakeNp,2)

    def test_nested_nonfinite_metadata_rejected(self):
        v.valid_json({'event_seq':3,'nested':[None,True,'text',.2]})
        with self.assertRaises(ValueError):v.valid_json({'nested':[float('inf')]})

    def test_gpu_imports_guarded_without_loading_packages(self):
        guard=v.NoGpu()
        for name in ('mlx.core','mlx_lm','coremltools','mlx_reservoir'):
            with self.assertRaises(ImportError):guard.find_spec(name)
        self.assertIsNone(guard.find_spec('numpy'))


class FileTests(unittest.TestCase):
    def setUp(self):
        temp=tempfile.TemporaryDirectory(prefix='fake-restore-',dir=HERE)
        self.addCleanup(temp.cleanup);self.root=Path(temp.name).resolve()
        # Fake sources are never imported; production constants are separately
        # checked by the source-binding test and the actual preparation.
        original = dict(v.UNCHANGED_SOURCE_SHA256)
        patcher = patch.dict(v.UNCHANGED_SOURCE_SHA256, {name: v.hashlib.sha256(
            b'# fake source never imported\n').hexdigest() for name in original})
        patcher.start();self.addCleanup(patcher.stop)

    def write(self,name,text='fake bytes'):
        path=self.root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text);return path

    def request(self):
        backup=self.root/'backup';backup.mkdir()
        (backup/'example.npz').write_bytes(b'fake bytes, not a numeric artifact')
        (backup/'example_thermostats.json').write_text('[]')
        inventory=v.state.inventory_tree(backup)
        source=self.root/'source';source.mkdir()
        sources={}
        for name in v.SOURCE_NAMES:
            p=source/name;p.write_text('# fake source never imported\n');sources[name]=v.digest(p)
        python=self.write('runtime/python','fake executable never executed')
        cfg=self.write('runtime/pyvenv.cfg','fake venv')
        interp={str(python):v.digest(python)}
        plan={'roles':{'reservoir':{'identity':{'argv':[str(python),'reservoir_service.py','--port','7881']}}},
              'input_hashes':{'reservoir_settings':'7'*64,'reservoir_candidate':sources['reservoir_service.py'],
                              'interpreter':v.state.canonical_hash(interp)}}
        context={'transaction_id':'fake-test','plan':plan,'bindings':{
            'plan_sha256':v.state.canonical_hash(plan),'provider_old_identity_sha256':'2'*64,
            'reservoir_old_identity_sha256':'3'*64,'repair_source_sha256':sources['reservoir_service.py'],
            'interpreter_sha256':v.state.canonical_hash(interp),'configuration_sha256':'7'*64}}
        witness=v.state.envelope(context,'backup',{'backup_root':str(backup),'backup_inventory':inventory})
        packages={}
        for name in ('numpy','torch','scikit-learn','websockets'):
            path=self.write('runtime/'+name+'/METADATA','fake package metadata')
            record=self.write('runtime/'+name+'/RECORD','fake package record')
            packages[name]={'version':'fake','files':{str(path):v.digest(path),str(record):v.digest(record)}}
        runtime={'python':str(python),'interpreter_files':interp,'packages':packages,
                 'pyvenv_cfg':{'path':str(cfg),'sha256':v.digest(cfg)}}
        return v.prepare_request(context,witness,source_manifest={'root':str(source),'files':sources},
                                 runtime_manifest=runtime,config={'input_dim':32,'n_nodes':192,'backend':'numpy'},
                                 work_root=v.RUNS/'fake-test-not-created')

    def test_prepare_is_explicitly_unexecuted_and_accepts_interpreter_group(self):
        request=self.request()
        self.assertFalse(request['passed']);self.assertFalse(Path(request['work_root']).exists())
        v.check_inputs(request)
        self.assertNotEqual(v.digest(request['runtime_manifest']['python']),request['bindings']['interpreter_sha256'])

    def test_live_or_reused_unsafe_paths_and_config_rejected(self):
        request=self.request()
        variants=[]
        r=copy.deepcopy(request);r['work_root']='/Users/v/other/neural-triple-reservoir/state';variants.append(r)
        r=copy.deepcopy(request);r['config']['n_nodes']=64;variants.append(r)
        r=copy.deepcopy(request);r['config']['backend']='mlx';variants.append(r)
        r=copy.deepcopy(request);r['backup_witness']['evidence']['backup_root']='/Users/v/other/neural-triple-reservoir/state';variants.append(r)
        r=copy.deepcopy(request);r['implementation']={};variants.append(r)
        r=copy.deepcopy(request);r['context']['plan']['input_hashes']['reservoir_settings']='8'*64;variants.append(r)
        for value in variants:
            with self.subTest(value=value),self.assertRaises((ValueError,FileNotFoundError)):v.validate_request(value)

    def test_input_hash_drift_rejected(self):
        request=self.request();p=Path(request['source_manifest']['root'])/'persistence.py';p.write_text('changed')
        with self.assertRaises(ValueError):v.check_inputs(request)

    def test_self_consistent_changed_supporting_source_rejected(self):
        request=self.request();p=Path(request['source_manifest']['root'])/'persistence.py'
        p.write_text('# altered supporting behavior\n')
        request['source_manifest']['files']['persistence.py']=v.digest(p)
        with self.assertRaisesRegex(ValueError,'supporting restore source'):
            v.validate_request(request)

    def test_backup_file_drift_rejected(self):
        request=self.request();p=Path(request['backup_witness']['evidence']['backup_root'])/'example.npz';p.write_text('changed')
        with self.assertRaises(ValueError):v.check_inputs(request)

    def test_independent_copy_keeps_every_file_and_file_timestamp(self):
        source=self.root/'original';source.mkdir();(source/'nested').mkdir()
        (source/'example.npz').write_bytes(b'fake');(source/'nested'/'invalid.keep').write_text('retain')
        before=v.state.inventory_tree(source)
        copied=v.copy_tree(source,self.root/'copy',before)
        self.assertEqual(v.file_content(before),v.file_content(copied))
        self.assertNotEqual(before['entries']['.']['inode'],copied['entries']['.']['inode'])
        self.assertEqual(v.state.inventory_tree(source),before)
        with self.assertRaises(FileExistsError):v.copy_tree(source,self.root/'copy',before)

    def accepted_fixture(self):
        request=self.request();v.RUNS.mkdir(exist_ok=True)
        temp=tempfile.TemporaryDirectory(prefix='fake-proof-',dir=v.RUNS);self.addCleanup(temp.cleanup)
        work=Path(temp.name);request['work_root']=str(work)
        req=self.write('request.json',json.dumps(request));log=work/'restore.log';log.write_text('SYNTHETIC UNIT TEST ONLY\n')
        names=['example'];startup={'example':{'backend':'numpy','entity':'fake','tick_count':7,'mode':'quiet'}}
        row={'passed':True,'source_fields':{key:{'dtype':'float32','shape':[1,192],'sha256':'a'*64} for key in v.NPZ_KEYS},
             'event_seq':7,'thermostat_semantics_sha256':'b'*64,'elapsed_time_checked':True}
        comparison={'passed':True,'errors':[],'warnings':[],'expected_restore_handles':names,'expected_startup':startup,
                    'comparisons':{'example':row},'pid':999,'started_wall':1.0,'completed_wall':2.0,
                    'post_restore_inventory':request['backup_witness']['evidence']['backup_inventory']}
        cp=work/'comparison.json';cp.write_text(json.dumps(comparison))
        evidence={'mode':'actual_private_restore','tasks_started':False,'listener_opened':False,'work_root':str(work),
                  'config':request['config'],'request':v.reference(req),'comparison':v.reference(cp),'log':v.reference(log),
                  'expected_restore_handles':names,'expected_startup':startup,'implementation':request['implementation'],
                  'source_manifest':request['source_manifest'],'runtime_manifest':request['runtime_manifest']}
        receipt=v.state.envelope(request['context'],'private_restore',evidence,extra_bindings=request['bindings'])
        path=work/'restore-witness.json';path.write_text(json.dumps(receipt))
        return request,path,receipt,comparison

    def test_witness_requires_complete_semantics_files_and_fresh_runtime(self):
        request,path,receipt,comparison=self.accepted_fixture()
        with patch.object(v,'check_runtime') as runtime:
            v.validate_restore_witness(path,request['context'],request['backup_witness'])
            runtime.assert_called_once_with(request,current=False)
        for change in ('missing_name','missing_field','missing_time','warnings','missing_inventory','artifact_drift'):
            altered=copy.deepcopy(comparison);witness=copy.deepcopy(receipt)
            if change=='missing_name':altered['comparisons']={}
            if change=='missing_field':altered['comparisons']['example']['source_fields'].pop('event_seq')
            if change=='missing_time':altered.pop('started_wall')
            if change=='warnings':altered['warnings']=['thermostat restore failed']
            if change=='missing_inventory':altered['post_restore_inventory']['entries'].pop('example.npz')
            if change=='artifact_drift':altered['completed_wall']=3.0
            cp=Path(witness['evidence']['comparison']['path']);cp.write_text(json.dumps(altered))
            if change!='artifact_drift':witness['evidence']['comparison']=v.reference(cp)
            path.write_text(json.dumps(witness))
            with self.subTest(change=change),patch.object(v,'check_runtime'),self.assertRaises((ValueError,KeyError)):
                v.validate_restore_witness(path,request['context'],request['backup_witness'])

    def test_false_or_incomplete_restore_witness_cannot_pass(self):
        request=self.request();path=self.write('restore-witness.json',json.dumps(v.state.envelope(
            request['context'],'private_restore',{'status':'not_executed'},passed=False)))
        with self.assertRaises(ValueError):v.validate_restore_witness(path,request['context'],request['backup_witness'])


class BoundaryTests(unittest.TestCase):
    def import_boundary(self):
        tree=ast.parse((HERE/'restore_verifier.py').read_text())
        execute=next(node for node in tree.body if isinstance(node,ast.FunctionDef)
                     and node.name=='execute_private_restore')
        construct=next(node for node in ast.walk(execute) if isinstance(node,ast.Assign)
                       and isinstance(node.value,ast.Call) and isinstance(node.value.func,ast.Attribute)
                       and node.value.func.attr=='ReservoirService')
        origins=next(node for node in ast.walk(execute) if isinstance(node,ast.For)
                     and isinstance(node.iter,ast.Name) and node.iter.id=='SOURCE_NAMES')
        return execute,construct,origins

    def test_complete_origin_check_follows_construction_before_state_use(self):
        execute,construct,origins=self.import_boundary()
        self.assertEqual(len(v.SOURCE_NAMES),6)
        self.assertIn('lsm_reservoir.py',v.SOURCE_NAMES)
        self.assertLess(construct.end_lineno,origins.lineno)
        parent=next(node for node in ast.walk(execute) if hasattr(node,'body')
                    and construct in getattr(node,'body',[]) and origins in node.body)
        self.assertEqual(parent.body.index(origins),parent.body.index(construct)+1)
        state_calls=[node for node in ast.walk(execute) if isinstance(node,ast.Call)
                     and ((isinstance(node.func,ast.Name) and node.func.id=='read_expected')
                          or (isinstance(node.func,ast.Attribute) and node.func.attr=='restore_all'))]
        self.assertTrue(state_calls)
        self.assertTrue(all(origins.end_lineno<node.lineno for node in state_calls))
        fresh=next(node for node in ast.walk(execute) if isinstance(node,ast.Call)
                   and any(isinstance(arg,ast.Constant) and arg.value=='service source was already imported'
                           for arg in node.args))
        self.assertLess(fresh.lineno,construct.lineno)

    def test_actual_origin_block_accepts_lazy_import_and_rejects_missing_or_shadow(self):
        _,construct,origins=self.import_boundary()
        block=ast.fix_missing_locations(ast.Module(body=[construct,origins],type_ignores=[]))
        source_dir=HERE/'fake-source-never-created'
        for variant in ('lazy_correct','lazy_missing','lazy_shadow'):
            modules={name[:-3]:SimpleNamespace(__file__=str(source_dir/name))
                     for name in v.SOURCE_NAMES if name!='lsm_reservoir.py'}
            events=[]
            def constructor(**kwargs):
                events.append('construct')
                if variant!='lazy_missing':
                    path=(source_dir/'lsm_reservoir.py' if variant=='lazy_correct'
                          else HERE/'shadow'/'lsm_reservoir.py')
                    modules['lsm_reservoir']=SimpleNamespace(__file__=str(path))
                return object()
            def checked_require(condition,message):
                events.append('origin_check');v.require(condition,message)
            namespace={'module':SimpleNamespace(ReservoirService=constructor),
                       'work':HERE/'fake-work-never-created','request':{'config':{}},
                       'SOURCE_NAMES':v.SOURCE_NAMES,'sys':SimpleNamespace(modules=modules),
                       'require':checked_require,'Path':Path,'source_dir':source_dir}
            with self.subTest(variant=variant):
                if variant=='lazy_correct':
                    exec(compile(block,'<actual-origin-boundary>','exec'),namespace)
                    self.assertEqual(events,['construct']+['origin_check']*6)
                else:
                    with self.assertRaisesRegex(ValueError,'wrong source import'):
                        exec(compile(block,'<actual-origin-boundary>','exec'),namespace)
                    self.assertEqual(events[0],'construct')

    def test_numeric_imports_are_inside_explicit_execute_only(self):
        tree=ast.parse((HERE/'restore_verifier.py').read_text())
        for node in tree.body:
            if isinstance(node,(ast.Import,ast.ImportFrom)):
                text=ast.unparse(node)
                self.assertFalse(any(name in text for name in ('numpy','torch','reservoir_service')))
        forbidden=[node for node in ast.walk(tree) if isinstance(node,ast.Call) and isinstance(node.func,ast.Attribute)
                   and node.func.attr in ('run_worker','rehearsal_loop','auto_snapshot_loop','serve','kill','terminate','send_signal')]
        self.assertEqual(forbidden,[])
        for name in ('numpy','torch','reservoir_service','mlx','coremltools'):
            self.assertNotIn(name,sys.modules)


if __name__=='__main__':unittest.main(verbosity=2)
