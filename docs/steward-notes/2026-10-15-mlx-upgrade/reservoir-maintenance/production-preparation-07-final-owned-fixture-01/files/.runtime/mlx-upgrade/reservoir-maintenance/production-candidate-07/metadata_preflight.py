"""Read-only live metadata peek. No stable inventory, backup or restore claim."""
from __future__ import annotations
import ast
from collections import Counter
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import plistlib
import re
import stat
import struct
import sys
import time
import zipfile

HERE=Path(__file__).resolve().parent
WORKSPACE=Path('/Users/v/other/neural-triple-reservoir')
PLIST=Path('/Users/v/Library/LaunchAgents/com.reservoir.service.plist')
MEMBER_LIMIT=4096
HEADER_LIMIT=2048
SIDECAR_LIMIT=1024*1024
ARCHIVE_LIMIT=256*1024*1024

class Inconclusive(Exception):pass

def identity(s):
    return {k:getattr(s,k) for k in ('st_dev','st_ino','st_size','st_mtime_ns')}

def require(ok,message):
    if not ok:raise ValueError(message)

def stable_read(path,reader):
    before=path.lstat()
    require(stat.S_ISREG(before.st_mode),'not a regular non-symlink file')
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW)
    result=None;error=None
    try:
        opened=os.fstat(fd)
        if identity(opened)!=identity(before):raise Inconclusive('changed between stat and open')
        try:
            with os.fdopen(fd,'rb',closefd=False) as stream:result=reader(stream,opened.st_size)
        except Exception as exc:error=exc
        after=os.fstat(fd)
        try:path_after=path.lstat()
        except OSError as exc:raise Inconclusive('path disappeared during read') from exc
        if identity(after)!=identity(before) or identity(path_after)!=identity(before):
            raise Inconclusive('inode/size/mtime changed during read; not retried')
        if error:raise error
        return result,identity(before)
    finally:os.close(fd)

def bounded_bytes(stream,size,limit):
    require(size<=limit,'file exceeds bounded read limit')
    data=stream.read(limit+1);require(len(data)==size and len(data)<=limit,'bounded read length differs')
    return data

def npy_member(archive,info,scalar=False):
    require(info.file_size<=MEMBER_LIMIT if scalar else info.file_size<=ARCHIVE_LIMIT,'NPY member size exceeds limit')
    with archive.open(info) as stream:
        prefix=stream.read(8);require(len(prefix)==8 and prefix[:6]==b'\x93NUMPY','invalid NPY magic')
        version=tuple(prefix[6:]);require(version in ((1,0),(2,0),(3,0)),'unsupported NPY format version')
        width=2 if version==(1,0) else 4
        encoded=stream.read(width);require(len(encoded)==width,'truncated NPY header length')
        length=int.from_bytes(encoded,'little');require(length<=HEADER_LIMIT,'NPY header exceeds bounded limit')
        header=stream.read(length);require(len(header)==length,'truncated NPY header')
        data=ast.literal_eval(header.decode('utf-8' if version==(3,0) else 'latin1').strip())
        require(set(data)=={'descr','fortran_order','shape'} and isinstance(data['descr'],str)
                and type(data['fortran_order']) is bool and isinstance(data['shape'],tuple)
                and all(type(n) is int and n>=0 for n in data['shape']),'invalid NPY header')
        result={'dtype':data['descr'],'shape':list(data['shape']),'fortran_order':data['fortran_order']}
        if scalar:
            require(data['shape']==() and not data['fortran_order'],'metadata must be scalar')
            payload=stream.read(MEMBER_LIMIT+1)
            require(8+width+length+len(payload)==info.file_size and len(payload)<=MEMBER_LIMIT,'metadata size differs')
            match=re.fullmatch(r'([<>])U([0-9]+)',data['descr'])
            if match:
                require(len(payload)==int(match[2])*4,'Unicode payload length differs')
                result['value']=payload.decode('utf-32-le' if match[1]=='<' else 'utf-32-be').rstrip('\0')
            else:
                require(data['descr'] in ('<i8','>i8') and len(payload)==8,'unsupported scalar metadata dtype')
                result['value']=int.from_bytes(payload,'little' if data['descr'][0]=='<' else 'big',signed=True)
        return result

def archive_metadata(stream,size):
    require(size<=ARCHIVE_LIMIT,'archive exceeds bounded limit')
    with zipfile.ZipFile(stream) as archive:
        infos=archive.infolist();require(len(infos)<=64,'too many archive members')
        members={i.filename:i for i in infos};require(len(members)==len(infos),'duplicate archive member')
        result={'members':sorted(members),'fields':{}}
        for name in ('backend','config_fingerprint','snapshot_version','h1','h2','h3','last_input'):
            require(name+'.npy' in members,'missing member '+name)
            result['fields'][name]=npy_member(archive,members[name+'.npy'],scalar=name in ('backend','config_fingerprint','snapshot_version'))
        return result

def sidecar_metadata(stream,size):
    value=json.loads(bounded_bytes(stream,size,SIDECAR_LIMIT),parse_constant=lambda s:(_ for _ in ()).throw(ValueError('nonfinite JSON')))
    keys={'entropy_target','rho','tick','last_entropy','last_saturation','warmup_entropies','buffer_tail'}
    require(isinstance(value,list) and len(value)==3,'exactly three thermostat entries required')
    rows=[]
    def finite(v):return type(v) in (int,float) and math.isfinite(v)
    for layer in value:
        require(isinstance(layer,dict) and set(layer)==keys,'thermostat fields incomplete/unknown')
        require(type(layer['tick']) is int and layer['tick']>=0 and finite(layer['rho']),'invalid thermostat tick/rho')
        require(all(layer[k] is None or finite(layer[k]) for k in ('entropy_target','last_entropy','last_saturation')),'invalid thermostat metric')
        require(isinstance(layer['warmup_entropies'],list) and all(finite(v) for v in layer['warmup_entropies']),'invalid warmup history')
        tail=layer['buffer_tail'];require(isinstance(tail,list) and all(isinstance(r,list) and len(r)==192 and all(finite(v) for v in r) for r in tail),'invalid thermostat row shape/content')
        rows.append({'fields':sorted(layer),'buffer_rows':len(tail),'warmup_entries':len(layer['warmup_entropies'])})
    return {'layers':rows,'bytes':size}

def main():
    started=datetime.datetime.now(datetime.timezone.utc).isoformat();start=time.monotonic()
    plist_bytes,plist_id=stable_read(PLIST,lambda f,n:bounded_bytes(f,n,65536));plist=plistlib.loads(plist_bytes)
    argv=plist['ProgramArguments'];state_root=Path(argv[argv.index('--state-dir')+1])
    require(state_root==WORKSPACE/'state','unexpected state root')
    require('--input-dim' not in argv and '--n-nodes' not in argv and '--backend' not in argv,'nondefault launch config requires separate reviewed parsing')
    sources={}
    for name in ('reservoir_service.py','triple_reservoir_coreml.py'):
        raw,ident=stable_read(WORKSPACE/name,lambda f,n:bounded_bytes(f,n,256*1024))
        sources[name]={'sha256':hashlib.sha256(raw).hexdigest(),'identity':ident,'ast':ast.parse(raw)}
    svc=sources['reservoir_service.py']['ast'];defaults=sources['triple_reservoir_coreml.py']['ast']
    constants={n.targets[0].id:ast.literal_eval(n.value) for n in svc.body if isinstance(n,ast.Assign) and len(n.targets)==1
               and isinstance(n.targets[0],ast.Name) and n.targets[0].id in ('SNAPSHOT_SCHEMA_VERSION','RESERVOIR_RUNTIME_VERSION')}
    cls=next(n for n in defaults.body if isinstance(n,ast.ClassDef) and n.name=='ReservoirConfig')
    config={n.target.id:ast.literal_eval(n.value) for n in cls.body if isinstance(n,ast.AnnAssign) and n.value is not None}
    config.update(input_dim=32,n_nodes=192,runtime_version=constants['RESERVOIR_RUNTIME_VERSION'])
    fingerprint=hashlib.sha256(json.dumps(config,sort_keys=True,separators=(',',':')).encode()).hexdigest()[:16]
    for row in sources.values():row.pop('ast')
    directory_before=identity(state_root.stat());names=sorted(p.name for p in state_root.iterdir() if p.name.endswith('.npz'))
    rows=[]
    for filename in names:
        path=state_root/filename;row={'name':path.stem,'issues':[]}
        try:
            metadata,ident=stable_read(path,archive_metadata);row.update(npz_identity=ident,metadata=metadata)
            fields=metadata['fields'];backend=fields['backend']['value'];version=fields['snapshot_version']['value'];fp=fields['config_fingerprint']['value']
            if backend not in ('','numpy','lsm_shadow'):row['issues'].append('unsupported_backend')
            if path.stem.endswith('__lsm') and backend not in ('','lsm_shadow'):row['issues'].append('shadow_backend_mismatch')
            if version!=constants['SNAPSHOT_SCHEMA_VERSION']:row['issues'].append('incompatible_snapshot_version')
            if fp!=fingerprint:row['issues'].append('incompatible_config_fingerprint')
            if any(fields[k]['shape']!=[1,192] or fields[k]['dtype']!='<f4' for k in ('h1','h2','h3')):row['issues'].append('incompatible_hidden_shape_dtype')
            if fields['last_input']['shape'] not in ([32],[1,32]):row['issues'].append('incompatible_last_input_shape')
            row['npz_status']='stable_read'
        except Inconclusive as exc:row.update(npz_status='inconclusive',npz_error=str(exc))
        except Exception as exc:row.update(npz_status='invalid_or_unreadable',npz_error=type(exc).__name__+': '+str(exc))
        try:
            metadata,ident=stable_read(state_root/(path.stem+'_thermostats.json'),sidecar_metadata)
            row.update(sidecar_status='stable_read',sidecar_identity=ident,sidecar=metadata)
        except Inconclusive as exc:row.update(sidecar_status='inconclusive',sidecar_error=str(exc))
        except FileNotFoundError:row.update(sidecar_status='missing')
        except Exception as exc:row.update(sidecar_status='invalid_or_unreadable',sidecar_error=type(exc).__name__+': '+str(exc))
        rows.append(row)
    def distribution(field):return dict(Counter(str(row['metadata']['fields'][field]['value']) for row in rows if row['npz_status']=='stable_read'))
    receipt={'schema':'reservoir_live_metadata_preflight_v1','created_at':started,'completed_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),
      'scope':'One-pass live read-only metadata/sidecar peek; not a stable final inventory, backup, or restore acceptance.',
      'stable_final_inventory':False,'backup_created':False,'restore_accepted':False,'state_root':str(state_root),
      'script':{'path':str(Path(__file__).resolve()),'sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
      'interpreter':{'path':sys.executable,'version':sys.version},'sources':sources,
      'launch_plist':{'path':str(PLIST),'sha256':hashlib.sha256(plist_bytes).hexdigest(),'identity':plist_id,'argv':argv},
      'expected':{'backend':'numpy','snapshot_version':constants['SNAPSHOT_SCHEMA_VERSION'],'config_fingerprint':fingerprint,'config':config},
      'limits':{'metadata_member_bytes':MEMBER_LIMIT,'header_bytes':HEADER_LIMIT,'sidecar_bytes':SIDECAR_LIMIT,'archive_bytes':ARCHIVE_LIMIT,'max_members':64,'retries':0},
      'read_guards':'lstat + O_NOFOLLOW open/fstat + final fstat/lstat compare dev/inode/size/mtime_ns for each file; unstable read is inconclusive without retry.',
      'directory_before':directory_before,'directory_after':identity(state_root.stat()),
      'summary':{'observed_npz_paths':len(rows),'npz_status':dict(Counter(r['npz_status'] for r in rows)),
                 'sidecar_status':dict(Counter(r['sidecar_status'] for r in rows)),
                 'backend_distribution':distribution('backend'),'snapshot_version_distribution':distribution('snapshot_version'),
                 'config_fingerprint_distribution':distribution('config_fingerprint'),'issues':dict(Counter(i for r in rows for i in r['issues']))},
      'items':rows,'elapsed_seconds':round(time.monotonic()-start,6),
      'mandatory_next_check':'Future exclusive stopped-state revalidation and actual private restore remain mandatory. Independently stable reads are not an atomic cross-file snapshot.',
      'not_tested':['Full numeric payload integrity/values','Actual package restore behavior','Cross-file point-in-time consistency','Production-memory equality']}
    target=HERE/'metadata-preflight-01.json'
    with target.open('x') as out:json.dump(receipt,out,indent=2,allow_nan=False);out.write('\n')
    print(json.dumps({'receipt':str(target),'sha256':hashlib.sha256(target.read_bytes()).hexdigest(),'summary':receipt['summary'],'elapsed_seconds':receipt['elapsed_seconds']},indent=2))

if __name__=='__main__':main()
