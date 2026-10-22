"""Explicit private actual-restore verifier. Default CLI only inspects its request.

Never opens a listener, schedules rehearsal, or points PersistenceManager at the
backup/live tree. Numeric imports occur only in execute_private_restore().
"""
from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.abc
import importlib.util
import json
import logging
import math
import os
from pathlib import Path
import shutil
import sys
import time
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import reservoir_maintenance_state as state

HERE = Path(__file__).resolve().parent
RUNS = HERE / "restore-runs"
MAINTENANCE = HERE.parent
ENV_HELPER = Path('/Users/v/other/astrid/scripts/model_environment.py')
SOURCE_NAMES = {'reservoir_service.py', 'persistence.py', 'dual_ai_bridge.py',
                'triple_reservoir_coreml.py', 'lsm_reservoir.py', 'rehearsal.py'}
# This verifier is for the reviewed one-file scheduling repair only. Supporting
# restore semantics must remain the exact originals, even with a self-consistent
# request/source manifest. The Host separately binds the full prepared closure.
UNCHANGED_SOURCE_SHA256 = {
    'dual_ai_bridge.py': 'a7adae36510c496bff084a10d41ecd7b40ffb4f0c259924a448b56bbfe17a549',
    'lsm_reservoir.py': '36143cc0d9ed1b377b251abc42f5a4957e9ef835a4739b1e8cd6404625957417',
    'persistence.py': 'b550833fdb819f38bbdabb6990e134e68629f6a63397f23b7b7afb2bed2ca72f',
    'rehearsal.py': 'bf3558a04c75f04d489653c71d74d55d378330153a69870705a5a393660b8093',
    'triple_reservoir_coreml.py': '524d44ec4c03f0be43236a635eeb2bb783f61ea5d3227a133bd451aadf6644a8',
}
NPZ_KEYS = {'h1', 'h2', 'h3', 'last_input', 'backend', 'mode', 'decay_profile', 'decay_weight',
            'tick_count', 'event_seq', 'snapshot_version', 'config_fingerprint', 'entity', 'timestamp',
            'last_live_wall_time', 'mode_authority', 'hint_policy', 'last_live_meta_json',
            'last_lane_meta_json', 'provenance_json', 'output_history', 'h_norm_history'}
THERMO_KEYS = {'entropy_target', 'rho', 'tick', 'last_entropy', 'last_saturation', 'warmup_entropies', 'buffer_tail'}
require = state.require


def digest(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def reference(path):
    path = Path(path).absolute()
    require(path.is_file() and not path.is_symlink(), 'artifact must be a regular non-symlink file')
    return {'path': str(path), 'sha256': digest(path)}


def read_json(path):
    data, _ = state.artifact(path)
    return json.loads(data, parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def implementation():
    return {str(path): digest(path) for path in (Path(__file__).resolve(), HERE/'reservoir_maintenance_state.py', ENV_HELPER)}


def verify_hashes(files):
    require(isinstance(files, dict) and files, 'empty artifact binding')
    for path, expected in files.items():
        require(isinstance(expected, str) and state.SHA.fullmatch(expected), 'invalid digest')
        require(Path(path).is_file() and not Path(path).is_symlink() and digest(path) == expected,
                'bound file changed: ' + str(path))


def file_content(inventory):
    return {name: ({key: item[key] for key in ('kind', 'bytes', 'sha256', 'mtime_ns')}
                   if item['kind']=='file' else {'kind':'directory'})
            for name, item in inventory['entries'].items()}


def validate_request(request):
    require(request.get('schema') == 'reservoir_private_restore_request_v1' and request.get('passed') is False,
            'only an unexecuted restore request is valid')
    context, backup = request['context'], request['backup_witness']
    require(backup.get('kind') == 'backup' and backup.get('passed') is True
            and backup.get('transaction_id') == context['transaction_id'], 'owned backup witness required')
    for key in state.COMMON:
        require(backup['bindings'].get(key) == context['bindings'].get(key), 'backup context mismatch')
    bindings = request['bindings']
    require(bindings['backup_sha256'] == state.canonical_hash(backup), 'backup witness hash mismatch')
    require(all(state.SHA.fullmatch(bindings.get(key, '')) for key in
                ('backup_sha256', 'repair_source_sha256', 'interpreter_sha256', 'configuration_sha256')),
            'restore bindings incomplete')
    work = Path(request['work_root'])
    require(work.is_absolute() and work.parent == RUNS and work.name not in ('', '.', '..')
            and work == work.resolve(), 'work root must be a canonical fresh direct child of private restore-runs')
    backup_root = Path(backup['evidence']['backup_root'])
    require(backup_root.is_relative_to(MAINTENANCE) and backup_root == backup_root.resolve(strict=True)
            and backup_root != work and not backup_root.is_relative_to(work), 'backup must be private and independent')
    require(backup['evidence']['backup_inventory']['root'] == str(backup_root), 'backup inventory root differs')
    source = request['source_manifest']
    require(set(source['files']) == SOURCE_NAMES and Path(source['root']).is_absolute(), 'complete source closure required')
    require(source['files']['reservoir_service.py'] == bindings['repair_source_sha256'], 'wrong repair source')
    require(all(source['files'][name] == sha for name, sha in UNCHANGED_SOURCE_SHA256.items()),
            'supporting restore source differs from reviewed original')
    config = request['config']
    require(set(config) == {'input_dim', 'n_nodes', 'backend'} and config['backend'] == 'numpy'
            and all(type(config[key]) is int and 0 < config[key] <= 4096 for key in ('input_dim', 'n_nodes')),
            'only explicit bounded NumPy service configuration is supported')
    runtime = request['runtime_manifest']
    require(Path(runtime['python']).is_absolute() and runtime['packages'], 'pinned runtime is required')
    require(state.canonical_hash(runtime['interpreter_files']) == bindings['interpreter_sha256'],
            'interpreter group differs from transaction')
    require(runtime['interpreter_files'].get(str(Path(runtime['python']).resolve(strict=True))) == digest(runtime['python']),
            'interpreter launcher changed or not bound')
    require({'numpy', 'torch', 'scikit-learn', 'websockets'} <= set(runtime['packages']), 'CPU packages missing')
    require(request['implementation'] == implementation(), 'verifier implementation changed')
    require(all(context['bindings'].get(key)==bindings[key] for key in
                ('repair_source_sha256','interpreter_sha256','configuration_sha256')), 'request differs from transaction bindings')
    require(state.canonical_hash(context['plan'])==bindings['plan_sha256'], 'transaction plan hash differs')
    require(bindings['configuration_sha256']==context['plan']['input_hashes']['reservoir_settings']
            and bindings['repair_source_sha256']==context['plan']['input_hashes']['reservoir_candidate']
            and bindings['interpreter_sha256']==context['plan']['input_hashes']['interpreter'], 'plan input bindings differ')
    argv=context['plan']['roles']['reservoir']['identity']['argv']
    def option(name,default):
        require(argv.count(name)<=1, 'duplicate reservoir option')
        return argv[argv.index(name)+1] if name in argv else default
    expected_config={'input_dim':int(option('--input-dim',32)), 'n_nodes':int(option('--n-nodes',192)),
                     'backend':option('--backend','numpy')}
    require(config==expected_config, 'restore config differs from preserved reservoir command')
    return request


def prepare_request(context, backup_witness, *, source_manifest, runtime_manifest, config, work_root):
    bindings = dict(context['bindings'])
    bindings.update(backup_sha256=state.canonical_hash(backup_witness))
    # These three exact values are supplied by the transaction, not inferred from mutable state.
    require(all(key in bindings for key in ('repair_source_sha256', 'interpreter_sha256', 'configuration_sha256')),
            'transaction restore bindings are required')
    request = {'schema': 'reservoir_private_restore_request_v1', 'passed': False,
               'context': context, 'bindings': bindings, 'backup_witness': backup_witness,
               'source_manifest': source_manifest, 'runtime_manifest': runtime_manifest,
               'config': config, 'work_root': str(Path(work_root).absolute()), 'implementation': implementation()}
    return validate_request(request)


def check_inputs(request):
    validate_request(request)
    source = request['source_manifest']
    verify_hashes({str(Path(source['root'])/name): sha for name, sha in source['files'].items()})
    runtime = request['runtime_manifest']
    verify_hashes(runtime['interpreter_files'])
    verify_hashes({runtime['pyvenv_cfg']['path']: runtime['pyvenv_cfg']['sha256']})
    for package in runtime['packages'].values():
        verify_hashes(package['files'])
    backup = request['backup_witness']['evidence']
    require(state.inventory_tree(backup['backup_root']) == backup['backup_inventory'], 'immutable backup changed')


def copy_tree(source, destination, expected):
    source, destination = Path(source), Path(destination)
    require(state.inventory_tree(source) == expected, 'copy input changed')
    destination.mkdir(mode=0o700)
    for name, item in expected['entries'].items():
        if name == '.':
            continue
        target = destination/name
        if item['kind'] == 'directory':
            target.mkdir(mode=0o700)
        else:
            with (source/name).open('rb') as src, target.open('xb') as dst:
                shutil.copyfileobj(src, dst, length=64*1024)
                dst.flush(); os.fsync(dst.fileno())
            os.utime(target, ns=(item['mtime_ns'], item['mtime_ns']))
            require(digest(target) == item['sha256'], 'private copy differs')
    copied = state.inventory_tree(destination)
    require(file_content(copied) == file_content(expected), 'private tree content differs')
    require(state.inventory_tree(source) == expected, 'copy source changed')
    return copied


def load_environment_helper():
    spec = importlib.util.spec_from_file_location('restore_pinned_environment_helper', ENV_HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def check_runtime(request, *, current=True):
    runtime = request['runtime_manifest']
    if current:require(sys.executable == runtime['python'], 'run verifier with the exact requested interpreter')
    helper = load_environment_helper()
    require({str(path) for path in helper.interpreter_paths(Path(runtime['python']))} == set(runtime['interpreter_files']),
            'both launcher/framework runtime files must be pinned')
    packages = {name: value['version'] for name, value in runtime['packages'].items()}
    artifacts = {path: sha for value in runtime['packages'].values() for path, sha in value['files'].items()}
    artifacts[runtime['pyvenv_cfg']['path']] = runtime['pyvenv_cfg']['sha256']
    helper.verify_packages(Path(runtime['python']), {'executable': runtime['python'], 'packages': packages}, packages, artifacts)


def finite(value):
    return type(value) in (int, float) and math.isfinite(value)


def thermostat_expected(value, floor, np, n_nodes, tail=48):
    require(isinstance(value, dict) and set(value) == THERMO_KEYS, 'thermostat fields incomplete/unknown')
    require(value['entropy_target'] is None or finite(value['entropy_target']), 'invalid entropy target')
    require(finite(value['rho']) and type(value['tick']) is int and value['tick'] >= 0, 'invalid thermostat rho/tick')
    require(all(value[key] is None or finite(value[key]) for key in ('last_entropy', 'last_saturation')),
            'invalid thermostat metric')
    warmup, rows = value['warmup_entropies'], value['buffer_tail']
    require(isinstance(warmup, list) and all(finite(v) for v in warmup), 'invalid thermostat warmup')
    require(isinstance(rows, list) and all(isinstance(row, list) and len(row) == n_nodes
            and all(finite(v) for v in row) for row in rows), 'invalid thermostat buffer')
    converted = [np.asarray(row, dtype=np.float32).round(6).tolist() for row in rows[-tail:]]
    require(all(all(finite(v) for v in row) for row in converted), 'float32 thermostat overflow')
    return {**value, 'entropy_target': None if value['entropy_target'] is None else max(value['entropy_target'], floor),
            'warmup_entropies': [float(v) for v in warmup[-tail:]], 'buffer_tail': converted}


def valid_json(value):
    if value is None or type(value) in (str,bool,int):return
    if type(value) is float:
        require(math.isfinite(value),'nonfinite event metadata');return
    if isinstance(value,list):
        for item in value:valid_json(item)
        return
    require(isinstance(value,dict) and all(isinstance(key,str) for key in value),'invalid JSON metadata')
    for item in value.values():valid_json(item)


def descriptor(np, value):
    array = np.asarray(value)
    require(array.dtype.kind not in 'OV', 'object/void arrays forbidden')
    return {'dtype': str(array.dtype), 'shape': list(array.shape), 'sha256': hashlib.sha256(array.tobytes()).hexdigest()}


def read_expected(np, module, service, directory, name):
    path = Path(directory)/(name+'.npz')
    with zipfile.ZipFile(path) as container:
        infos=container.infolist()
        require(len(infos)==len(NPZ_KEYS) and {item.filename for item in infos}=={key+'.npy' for key in NPZ_KEYS}
                and sum(item.file_size for item in infos)<=state.MAX_FILE_BYTES, 'NPZ archive size/member bounds differ')
    with np.load(path, allow_pickle=False) as archive:
        require(set(archive.files) == NPZ_KEYS and len(archive.files) == len(NPZ_KEYS), 'NPZ field set differs')
        raw = {key: archive[key] for key in archive.files}
    n, d = service.config.n_nodes, service.config.input_dim
    for key in ('h1', 'h2', 'h3'):
        require(raw[key].dtype == np.float32 and raw[key].shape == (1,n) and bool(np.isfinite(raw[key]).all()), 'invalid hidden state')
    require(raw['last_input'].shape in ((d,), (1,d)) and raw['last_input'].dtype.kind == 'f'
            and bool(np.isfinite(raw['last_input']).all()), 'invalid last input')
    for key in ('output_history', 'h_norm_history'):
        require(raw[key].dtype == np.float32 and bool(np.isfinite(raw[key]).all()), 'invalid history')
    require(raw['output_history'].ndim == 1 and (raw['h_norm_history'].shape == (0,)
            or raw['h_norm_history'].ndim == 2 and raw['h_norm_history'].shape[1] == 3), 'invalid history shape')
    strings = ('backend','mode','decay_profile','config_fingerprint','entity','mode_authority','hint_policy',
               'last_live_meta_json','last_lane_meta_json','provenance_json')
    for key in strings:
        require(raw[key].shape == () and raw[key].dtype.kind == 'U', 'invalid string scalar: '+key)
    for key in ('tick_count','event_seq','snapshot_version'):
        require(raw[key].shape == () and raw[key].dtype == np.int64 and int(raw[key]) >= 0, 'invalid integer scalar')
    for key in ('decay_weight','timestamp','last_live_wall_time'):
        require(raw[key].shape == () and raw[key].dtype == np.float64 and finite(float(raw[key])), 'invalid float scalar')
    require(int(raw['snapshot_version']) == module.SNAPSHOT_SCHEMA_VERSION and str(raw['config_fingerprint']) == service._snapshot_fingerprint,
            'snapshot schema/fingerprint incompatible')
    stored_backend=str(raw['backend'])
    require(stored_backend in ('','numpy','lsm_shadow'), 'stored backend not covered by this CPU restore verifier')
    backend=stored_backend or ('lsm_shadow' if name.endswith(module.SHADOW_SUFFIX) else service.bridge.default_backend)
    require(not name.endswith(module.SHADOW_SUFFIX) or backend=='lsm_shadow', 'shadow backend mismatch')
    require(str(raw['mode']) in ('quiet','hold','rehearse') and str(raw['decay_profile']) in ('fast','medium','slow')
            and str(raw['mode_authority']) in ('system','hint','explicit') and str(raw['hint_policy']) in ('off','guarded'), 'invalid rehearsal metadata')
    metadata = {key: json.loads(str(raw[key]), parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite metadata')))
                for key in ('last_live_meta_json','last_lane_meta_json','provenance_json')}
    valid_json(metadata)
    live, lanes, provenance = (metadata[key] for key in ('last_live_meta_json','last_lane_meta_json','provenance_json'))
    require(isinstance(live,dict) and isinstance(lanes,dict) and isinstance(provenance,list)
            and all(isinstance(row,dict) for row in provenance), 'invalid event metadata')
    event_seq = max(int(raw['event_seq']), int(live.get('event_seq',0) or 0),
                    max((int(row.get('event_seq',0) or 0) for row in provenance),default=0))
    sidecar = read_json(Path(directory)/(name+'_thermostats.json'))
    require(isinstance(sidecar,list) and len(sidecar) == 3, 'exactly three thermostat sidecars required')
    thermostats = [thermostat_expected(row, module.LAYER_H_TARGET_FLOORS[i], np, n, module.THERMOSTAT_PERSIST_STATES)
                   for i,row in enumerate(sidecar)]
    return {'raw':raw, 'field_descriptors':{key:descriptor(np,value) for key,value in raw.items()},
            'live':live, 'lanes':lanes, 'provenance':provenance[-module.PROVENANCE_SIZE:], 'event_seq':event_seq,
            'thermostats':thermostats, 'startup':{key: str(raw[key]) for key in ('entity','mode')} | {'backend':backend,'tick_count':int(raw['tick_count'])}}


def compare_handle(np, module, service, name, expected, clocks):
    info, rs, raw = service.handles[name], service.rehearsal.get_state(name), expected['raw']
    require(rs is not None, 'rehearsal state missing')
    for key, actual in zip(('h1','h2','h3'),service.bridge.read_state(name)):
        require(descriptor(np,actual) == descriptor(np,raw[key]), 'restored hidden state differs: '+name+':'+key)
    require(descriptor(np,rs.last_live_input) == descriptor(np,raw['last_input']), 'last input differs')
    require(info.backend == expected['startup']['backend'] and info.entity == str(raw['entity'])
            and info.tick_count == int(raw['tick_count']) and info.event_seq == expected['event_seq'], 'handle metadata differs')
    require(info.last_live_meta == expected['live'] and info.last_lane_meta == expected['lanes']
            and list(info.provenance_ring) == expected['provenance'], 'event metadata differs')
    require(list(info.output_ring) == raw['output_history'][-module.RING_SIZE:].tolist()
            and list(info.h_norm_ring) == raw['h_norm_history'][-module.RING_SIZE:].tolist(), 'bounded history differs')
    require(all(getattr(rs,key) == str(raw[key]) for key in ('mode','decay_profile','mode_authority','hint_policy'))
            and rs.decay_weight == float(raw['decay_weight']), 'rehearsal metadata differs')
    require(len(info.thermostats) == 3 and state.canonical_hash([t.state_dict() for t in info.thermostats])
            == state.canonical_hash(expected['thermostats']), 'thermostat semantics differ')
    last = float(raw['last_live_wall_time'])
    if last >= 0:
        for actual, floor in ((info.last_tick_time,1e-6),(rs.last_live_time,0.0)):
            low = max(floor, clocks['mono_before']-max(0.0,clocks['wall_after']-last))
            high = max(floor, clocks['mono_after']-max(0.0,clocks['wall_before']-last))
            require(low-.05 <= actual <= high+.05, 'elapsed-time restore semantics differ')
    else:
        require(info.last_tick_time == 0 and clocks['mono_before'] <= rs.last_live_time <= clocks['mono_after'], 'missing-age defaults differ')
    return {'passed':True, 'source_fields':expected['field_descriptors'], 'event_seq':expected['event_seq'],
            'thermostat_semantics_sha256':state.canonical_hash(expected['thermostats']), 'elapsed_time_checked':True}


class NoGpu(importlib.abc.MetaPathFinder):
    def find_spec(self, fullname, path=None, target=None):
        if fullname.split('.')[0] in {'mlx','mlx_lm','mlx_reservoir','coremltools'}:
            raise ImportError('GPU/model import forbidden in CPU restore verifier: '+fullname)


def execute_private_restore(request_path):
    request_path = Path(request_path).resolve()
    request = validate_request(read_json(request_path))
    work = Path(request['work_root'])
    work.parent.mkdir(mode=0o700,exist_ok=True)
    work.mkdir(mode=0o700)  # never replace an earlier run
    log_path, comparison_path = work/'restore.log', work/'comparison.json'
    result = {'passed':False, 'expected_restore_handles':[], 'expected_startup':{}, 'comparisons':{},
              'started_wall':time.time(), 'pid':os.getpid(), 'argv':sys.argv, 'errors':[], 'warnings':[]}
    copied = None
    request_ref=reference(request_path)
    try:
        check_inputs(request);check_runtime(request)
        source_dir = work/'source'; source_dir.mkdir()
        for name, sha in request['source_manifest']['files'].items():
            target=source_dir/name; shutil.copyfile(Path(request['source_manifest']['root'])/name,target)
            require(digest(target)==sha,'source copy differs'); target.chmod(0o444)
        source_dir.chmod(0o555)
        backup=request['backup_witness']['evidence']
        original=copy_tree(backup['backup_root'],work/'input-state',backup['backup_inventory'])
        copied=copy_tree(work/'input-state',work/'restore-state',original)
        names=sorted(Path(name).stem for name,item in original['entries'].items()
                     if item['kind']=='file' and '/' not in name and name.endswith('.npz'))
        require(all(name and '/' not in name and '..' not in name for name in names), 'unsafe snapshot names')
        result['expected_restore_handles']=names
        require(not any(name[:-3] in sys.modules for name in SOURCE_NAMES), 'service source was already imported')
        sys.pycache_prefix=str(work/'unused-bytecode')
        sys.meta_path.insert(0,NoGpu());sys.path.insert(0,str(source_dir))
        def deny_network(event,args):
            if event in {'socket.bind','socket.connect','socket.sendto'}:
                raise RuntimeError('network activity forbidden in private restore verifier')
        sys.addaudithook(deny_network)
        with log_path.open('x') as log, contextlib.redirect_stdout(log), contextlib.redirect_stderr(log):
            import reservoir_service as module
            import numpy as np
            import torch
            torch.set_num_threads(1);torch.set_default_device('cpu')
            require(not any(name.split('.')[0] in {'mlx','mlx_lm','mlx_reservoir','coremltools'} for name in sys.modules), 'forbidden import occurred')
            service=module.ReservoirService(state_dir=work/'restore-state',**request['config'])
            # ReservoirBridge constructs the LSM backend here, importing its source lazily.
            # Check the complete source closure before reading or restoring stored state.
            for filename in SOURCE_NAMES:
                loaded=sys.modules.get(filename[:-3]);require(loaded is not None and Path(loaded.__file__).resolve()==source_dir/filename,'wrong source import')
            for name in names:
                expected=read_expected(np,module,service,work/'input-state',name)
                result['expected_startup'][name]=expected['startup']
            class Capture(logging.Handler):
                def emit(self,entry):
                    if entry.levelno>=logging.WARNING:result['warnings'].append(entry.getMessage())
            capture=Capture();logging.getLogger().addHandler(capture)
            try:
                clocks={'wall_before':time.time(),'mono_before':time.monotonic()}
                service.restore_all()
                clocks.update(wall_after=time.time(),mono_after=time.monotonic())
            finally:logging.getLogger().removeHandler(capture)
            require(not result['warnings'],'actual restore emitted warning/error')
            require(set(service.handles)==set(names),'actual restored name set differs')
            for name in names:
                expected=read_expected(np,module,service,work/'input-state',name)
                result['comparisons'][name]=compare_handle(np,module,service,name,expected,clocks)
        require(state.inventory_tree(work/'input-state')==original,'private input state changed')
        require(state.inventory_tree(work/'restore-state')==copied,'restore mutated/quarantined stored files')
        check_inputs(request);check_runtime(request)
        require(reference(request_path)==request_ref,'restore request changed during execution')
        require(all(digest(source_dir/name)==sha for name,sha in request['source_manifest']['files'].items()),'private source changed')
        result['passed']=True
    except BaseException as error:
        result['errors'].append(type(error).__name__+': '+str(error))
    finally:
        result['completed_wall']=time.time()
        if (work/'restore-state').is_dir():
            try:result['post_restore_inventory']=state.inventory_tree(work/'restore-state')
            except BaseException as error:result['errors'].append('post-inventory: '+str(error));result['passed']=False
        comparison_path.write_text(json.dumps(result,indent=2,allow_nan=False)+'\n')
        if not log_path.exists():log_path.write_text('Restore failed before numeric import; see comparison.json.\n')
        evidence={'mode':'actual_private_restore','expected_restore_handles':result['expected_restore_handles'],
                  'expected_startup':result['expected_startup'],'request':request_ref,
                  'comparison':reference(comparison_path),'log':reference(log_path),'implementation':request['implementation'],
                  'source_manifest':request['source_manifest'],'runtime_manifest':request['runtime_manifest'],
                  'config':request['config'],'work_root':str(work),'tasks_started':False,'listener_opened':False,
                  'evidence_limit':'Deterministic isolated restore inference; not direct production-memory equality.',
                  'coverage':'Complete stored public snapshot/metadata/thermostat semantics for NumPy/LSM; no promise of unstored LSM internal trajectories.'}
        receipt=state.envelope(request['context'],'private_restore',evidence,extra_bindings=request['bindings'],passed=result['passed'])
        (work/'restore-witness.json').write_text(json.dumps(receipt,indent=2,allow_nan=False)+'\n')
    return receipt


def validate_restore_witness(path, context, backup_witness):
    witness=read_json(path)
    require(witness.get('schema')=='reservoir_maintenance_witness_v1' and witness.get('kind')=='private_restore'
            and witness.get('passed') is True and witness.get('transaction_id')==context['transaction_id'], 'restore not accepted')
    require(all(witness['bindings'].get(k)==v for k,v in context['bindings'].items())
            and witness['bindings']['backup_sha256']==state.canonical_hash(backup_witness),'restore bindings differ')
    evidence=witness['evidence'];require(evidence.get('mode')=='actual_private_restore'
            and evidence.get('tasks_started') is False and evidence.get('listener_opened') is False,'wrong restore scope')
    work=Path(evidence['work_root'])
    require(work.parent==RUNS and Path(path).resolve()==work/'restore-witness.json','witness must belong to its private run')
    require(Path(evidence['comparison']['path'])==work/'comparison.json' and Path(evidence['log']['path'])==work/'restore.log',
            'restore artifacts outside own run')
    for key in ('request','comparison','log'):
        verify_hashes({evidence[key]['path']:evidence[key]['sha256']})
    request=validate_request(read_json(evidence['request']['path']));check_inputs(request);check_runtime(request,current=False)
    require(request['backup_witness']==backup_witness and request['bindings']==witness['bindings']
            and request['work_root']==evidence['work_root'] and request['config']==evidence['config'],'request provenance differs')
    comparison=read_json(evidence['comparison']['path'])
    names=evidence['expected_restore_handles']
    stored=sorted(Path(name).stem for name,item in backup_witness['evidence']['backup_inventory']['entries'].items()
                  if item['kind']=='file' and '/' not in name and name.endswith('.npz'))
    require(names==stored,'expected restore names differ from complete backup inventory')
    require(isinstance(names,list) and names==sorted(set(names)) and comparison.get('passed') is True
            and not comparison.get('errors') and not comparison.get('warnings'),'restore comparison failed')
    require(names==comparison['expected_restore_handles'] and set(comparison['comparisons'])==set(names)
            and all(row.get('passed') is True for row in comparison['comparisons'].values())
            and evidence['expected_startup']==comparison['expected_startup'] and set(evidence['expected_startup'])==set(names), 'restore coverage incomplete')
    require(type(comparison.get('pid')) is int and comparison['pid']>1
            and finite(comparison.get('started_wall')) and finite(comparison.get('completed_wall'))
            and comparison['completed_wall']>=comparison['started_wall'], 'actual execution identity/timing missing')
    require(file_content(comparison['post_restore_inventory'])==file_content(backup_witness['evidence']['backup_inventory']),
            'post-restore file preservation differs')
    for name,row in comparison['comparisons'].items():
        require(set(row.get('source_fields',{}))==NPZ_KEYS and row.get('elapsed_time_checked') is True
                and type(row.get('event_seq')) is int and row['event_seq']>=0
                and state.SHA.fullmatch(row.get('thermostat_semantics_sha256','')), 'incomplete semantic comparison')
        for field in row['source_fields'].values():
            require(isinstance(field.get('dtype'),str) and isinstance(field.get('shape'),list)
                    and all(type(n) is int and n>=0 for n in field['shape'])
                    and state.SHA.fullmatch(field.get('sha256','')), 'invalid array descriptor')
        startup=evidence['expected_startup'][name]
        require(set(startup)=={'entity','backend','tick_count','mode'} and startup['backend'] in ('numpy','lsm_shadow')
                and isinstance(startup['entity'],str) and type(startup['tick_count']) is int and startup['tick_count']>=0
                and startup['mode'] in ('quiet','hold','rehearse'), 'invalid startup expectation')
    require(evidence['implementation']==request['implementation'] and evidence['source_manifest']==request['source_manifest']
            and evidence['runtime_manifest']==request['runtime_manifest'], 'witness implementation/input provenance differs')
    return witness


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request',type=Path,required=True)
    parser.add_argument('--execute-private-restore',action='store_true')
    args=parser.parse_args()
    if not args.execute_private_restore:
        request=validate_request(read_json(args.request))
        print(json.dumps({'passed':False,'status':'not_executed','work_root':request['work_root']}));return 0
    receipt=execute_private_restore(args.request)
    print(json.dumps({'passed':receipt['passed'],'witness':str(Path(receipt['evidence']['work_root'])/'restore-witness.json')}))
    return 0 if receipt['passed'] else 1


if __name__=='__main__':raise SystemExit(main())
