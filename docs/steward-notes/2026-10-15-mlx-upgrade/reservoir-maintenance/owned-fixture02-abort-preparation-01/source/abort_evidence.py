"""Inert retained-file proofs for one failed synthetic fixture; stdlib only."""
from __future__ import annotations
import hashlib
import json
import math
import os
from pathlib import Path
import stat

HERE = Path(__file__).resolve().parent
MAINTENANCE = HERE.parent
TOKEN = '4327212894d64b31980b98bf3fbd7fc2'
RUN = MAINTENANCE/'owned-recovery-fixture-02/runs'/('owned-recovery-'+TOKEN)
ABORT = MAINTENANCE/'evidence'/('owned-fixture02-abort-'+TOKEN+'-01')
SNAPSHOT_SHA = 'c6c11df7fb4bc18d4ead51df32c8eeae6635da187cd2e127f1a62d053c8a36a2'
RELEASE_SHA = 'aa1273e375e376d51ea70be44b1bbb684f1a0a5c3fc96a7ca0bb5bc192ee215c'
STATE = {'fixture_astrid': {'ticks': 11}, 'fixture_minime': {'ticks': 7}}
LIMIT = 16*1024*1024


def require(value, message):
    if not value: raise ValueError(message)


def digest(value):
    return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':'),allow_nan=False).encode()).hexdigest()


def stamp(info):
    return (info.st_dev,info.st_ino,info.st_size,info.st_mtime_ns,info.st_ctime_ns,
            info.st_mode,info.st_uid,info.st_nlink)


def directory(path):
    path=Path(path); info=path.lstat()
    require(path.is_absolute() and path==path.resolve(strict=True) and stat.S_ISDIR(info.st_mode)
            and info.st_uid==os.getuid() and not stat.S_IMODE(info.st_mode)&0o077,'unsafe private directory')
    return path


def raw(path, limit=LIMIT):
    path=Path(path)
    require(path.is_absolute() and path==path.resolve(strict=True),'noncanonical retained file')
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
    try:
        before=os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and before.st_uid==os.getuid() and before.st_nlink==1
                and before.st_size<=limit,'unsafe or oversized retained file')
        chunks=[]; size=0
        while value:=os.read(fd,min(1024*1024,limit+1-size)):
            size+=len(value); require(size<=limit,'retained file grew'); chunks.append(value)
        require(stamp(before)==stamp(os.fstat(fd))==stamp(path.lstat()),'retained file changed during read')
        return b''.join(chunks)
    finally: os.close(fd)


def sha(path):
    """Bounded streaming source/Python/protected-input hash; no package import."""
    path=Path(path); require(path.is_absolute() and path==path.resolve(strict=True),'noncanonical hash input')
    fd=os.open(path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
    try:
        before=os.fstat(fd); require(stat.S_ISREG(before.st_mode) and before.st_nlink==1
            and before.st_size<=1024*1024*1024,'unsafe/oversized hash input')
        h=hashlib.sha256(); count=0
        while data:=os.read(fd,1024*1024):
            count+=len(data); require(count<=1024*1024*1024,'hash input grew'); h.update(data)
        require(stamp(before)==stamp(os.fstat(fd))==stamp(path.lstat()),'hash input changed')
        return h.hexdigest()
    finally: os.close(fd)


def decode(data):
    def pairs(values):
        result={}
        for key,value in values:
            require(key not in result,'duplicate JSON key');result[key]=value
        return result
    return json.loads(data,object_pairs_hook=pairs,parse_constant=lambda _:require(False,'nonfinite JSON'))


def read(path):
    result=decode(raw(path));require(isinstance(result,dict),'JSON object required');return result


def reference(path): return {'path':str(path),'sha256':sha(path)}


def checked(ref):
    require(isinstance(ref,dict) and set(ref)=={'path','sha256'},'exact reference required')
    data=raw(Path(ref['path']));require(hashlib.sha256(data).hexdigest()==ref['sha256'],'reference hash differs')
    value=decode(data);require(isinstance(value,dict),'reference object required');return value


def identity(value):
    require(isinstance(value,dict) and set(value)=={'pid','started_at','argv'} and type(value['pid']) is int
            and value['pid']>1 and isinstance(value['started_at'],str) and value['started_at']
            and isinstance(value['argv'],list) and value['argv'] and all(isinstance(v,str) and v for v in value['argv']),
            'exact process identity required')
    return value


def source_artifacts():
    return {str(p):sha(p) for p in sorted(HERE.glob('*.py'))} | {
        str(HERE/'target.json'):sha(HERE/'target.json'),str(HERE/'README.md'):sha(HERE/'README.md')}


def target():
    t=read(HERE/'target.json')
    require(t['schema']=='owned_fixture02_abort_target_v1' and t['token']==TOKEN and t['run_root']==str(RUN)
            and t['abort_root']==str(ABORT) and t['snapshot']['sha256']==SNAPSHOT_SHA
            and t['release']['sha256']==RELEASE_SHA,'different abort target')
    release=checked(t['release']); snapshot=checked(t['snapshot']); observation=checked(t['observation'])
    require(snapshot['schema']=='owned_fixture_failed_run_snapshot_v1' and snapshot['run_root']==str(RUN)
            and snapshot['failed_run_remains_failed'] is True and not snapshot['per_file_read_changes']
            and len(snapshot['entries'])==141,'failed snapshot differs')
    plan=read(RUN/'plan.json'); prep=read(RUN/'preparation.json'); parent=read(RUN/'parent-start.json')
    require(t['host_binding']==plan['host_binding']==release['host_binding']==parent['host_binding']
            and t['parent_identity']==parent['identity'] and t['workers']=={r:plan['roles'][r]['identity'] for r in ('provider','reservoir')}
            and t['ports']=={r:prep['anchors'][r]['sentinel_port'] for r in ('provider','reservoir')}
            and t['targets']==release['fixture']['targets'] and t['coordination']==release['coordination']
            and t['python']==release['python'] and t['python_files']==release['python_files'], 'original identities/inputs differ')
    require(t['workers']['provider']['pid']==9267 and t['workers']['reservoir']['pid']==9217
            and t['provider_holder']['pid']==10721 and t['parent_identity']['pid']==9175
            and t['protected_exact']==observation['protected_identity']['protected_exact']
            and t['protected_files']==observation['protected_identity']['identity_files'], 'fixed participant/protected identity differs')
    require(t['provider_guard']=={'schema':'model_environment_hold_v1','transaction_id':TOKEN,'actor':'owned-fixture-only'}
            and t['reservoir_guard']=={'schema':'model_environment_hold_v1','transaction_id':TOKEN+'-abort-01','actor':'owned-fixture-abort-only'},'abort guard differs')
    for role in ('provider','reservoir','pin'):
        require(t['targets'][role]==f'gui/501/com.codex.reservoir-maintenance-fixture.{TOKEN}.{role}','foreign target')
    return t,snapshot,release


def verify_sources(t, release):
    resolved=Path(t['python']).resolve(strict=True);family={str(resolved)}
    require(resolved.parent.name=='bin' and resolved.parent.parent.parent.name=='Versions'
            and resolved.parent.parent.parent.parent.name=='Python.framework','selected Python family differs')
    family.add(str((resolved.parent.parent/'Resources/Python.app/Contents/MacOS/Python').resolve(strict=True)))
    require(family==set(t['python_files']) and str(resolved).endswith('/Versions/3.14/bin/python3.14'),
            'selected Python alias was retargeted')
    maps=[release['core']['artifacts'],release['artifacts'],t['python_files'],t['protected_files']]
    combined={}
    for values in maps:
        for path,value in values.items():
            require(path not in combined or combined[path]==value,'conflicting immutable binding');combined[path]=value
    require(len(combined)<500,'source closure too broad')
    for path,value in combined.items(): require(sha(Path(path))==value,'immutable source/runtime changed: '+path)
    checked(t['design']); checked(t['observation'])
    return {'count':len(combined),'sha256':digest(combined)}


def files_under(root):
    directory(root); result=set(); count=0; size=0
    for parent,dirs,names in os.walk(root,followlinks=False):
        count+=1;require(count<=500,'too many retained directories');directory(Path(parent))
        for name in dirs: directory(Path(parent)/name)
        for name in names:
            p=Path(parent)/name;info=p.lstat();require(stat.S_ISREG(info.st_mode) and info.st_nlink==1
                and info.st_uid==os.getuid(),'unsafe retained run file')
            result.add(str(p));size+=info.st_size
            require(len(result)<=500 and size<=32*1024*1024,'retained run exceeds finite bound')
    return result


def verify_original(t, snapshot, *, saved=False, provider_released=False):
    """Exact old file set; only explicit normal worker lifecycle differences."""
    root=Path(t['run_root']); archive=Path(t['snapshot']['path']).parent; entries=snapshot['entries']
    expected={row['source']:row for row in entries}; require(len(expected)==141,'duplicate snapshot entry')
    wanted=set(expected)
    if saved: wanted.discard(str(root/'reservoir.ready.json'));wanted.add(str(root/'state.json'))
    if provider_released: wanted.discard(str(root/'hold-provider/hold.json'))
    require(files_under(root)==wanted,'original run record set changed')
    for path,row in expected.items():
        require(Path(path).is_relative_to(root) and row['stable_during_read'] is True,'foreign snapshot entry')
        original=archive/row['snapshot']; require(original.is_relative_to(archive/'files') and sha(original)==row['sha256'], 'preserved snapshot changed')
        if saved and path==str(root/'reservoir.ready.json'): continue
        if provider_released and path==str(root/'hold-provider/hold.json'): continue
        if saved and path==str(root/'reservoir.events.jsonl'):
            data=raw(Path(path)); info=Path(path).stat()
            require({'device':info.st_dev,'inode':info.st_ino}==t['reservoir_log_identity']
                    and len(data)>=row['bytes'] and hashlib.sha256(data[:row['bytes']]).hexdigest()==row['sha256'],
                    'original reservoir log prefix/inode changed')
        else: require(sha(Path(path))==row['sha256'],'original retained bytes changed: '+path)
    require(saved or not (root/'state.json').exists(),'unexpected pre-abort state file')
    return {'original_snapshot':t['snapshot'],'original_file_count':len(expected),'saved':saved,'provider_guard_released':provider_released}


def event_rows(path):
    data=raw(path);require(data.endswith(b'\n'),'partial worker log')
    result=[decode(line) for line in data.splitlines()]
    require(all(isinstance(row,dict) for row in result),'invalid worker log');return result


def checkin_proof(t, *, saved=False):
    root=Path(t['run_root']); request_id=t['request_id']; provider=event_rows(root/'provider.events.jsonl')
    reservoir=event_rows(root/'reservoir.events.jsonl')
    pp=['started','idle','generation_admitted','stop_received','checkin_sent','checkin_acknowledged','drained']
    rp=['started','restored','checkin_applied']
    if saved:rp+=['stop_received','final_snapshot_started','handle_saved','handle_saved','final_snapshot_complete','drained']
    for role,rows,phases in (('provider',provider,pp),('reservoir',reservoir,rp)):
        ident=t['workers'][role]
        require([r.get('phase') for r in rows]==phases and all(r.get('token')==TOKEN and r.get('role')==role
                and r.get('pid')==ident['pid'] for r in rows),'unexpected worker lifecycle')
        require(rows[0]['argv']==ident['argv'][1:] and rows[0]['cwd']==str(root),'worker source/argv differs')
        times=[r.get('monotonic') for r in rows]
        require(all(type(v) in (int,float) and math.isfinite(v) for v in times) and times==sorted(times),'worker timestamps differ')
    require(all(r.get('request_id')==request_id for r in provider[2:]) and provider[2]['ticks']==provider[5]['ticks']==11
            and provider[4]['monotonic']-provider[3]['monotonic']>=5.9,'provider did not complete single admitted check-in')
    require(reservoir[1]['state']=={'fixture_astrid':{'ticks':0},'fixture_minime':{'ticks':7}}
            and reservoir[2]['state']==STATE and reservoir[2]['request_id']==request_id,'reservoir applied check-in differs')
    request=root/('request.'+request_id+'.json');ack=root/('ack.'+request_id+'.json')
    require(list(root.glob('request.*.json'))==[request] and list(root.glob('ack.*.json'))==[ack],'extra request or ACK')
    require(read(request)=={'token':TOKEN,'request_id':request_id,'handle':'fixture_astrid','ticks':11,'provider_pid':9267}
            and read(ack)=={'token':TOKEN,'request_id':request_id,'ticks':11,'ok':True},'request/ACK changed')
    if saved:
        require(reservoir[4]['monotonic']-reservoir[3]['monotonic']>=5.9
                and reservoir[4]['handles']==sorted(STATE)
                and [(r['name'],r['value']) for r in reservoir[5:7]]==sorted(STATE.items())
                and reservoir[7]['state']==STATE and read(root/'state.json')==STATE,'final durable11/7 save differs')
    return {'request':reference(request),'ack':reference(ack),'provider_log':reference(root/'provider.events.jsonl'),
            'reservoir_log':reference(root/'reservoir.events.jsonl'),'state':reference(root/'state.json') if saved else None,
            'expected_state':STATE,'saved':saved,'one_unique_checkin':True}
