"""Pure exact fixture03 native-chain reconstruction; never imports OS adapters."""
from __future__ import annotations
import json
import os
import re
from pathlib import Path
import stat

from production_fixture_admission import (FIXTURE,require,digest,sha_value,finite,process_matches)

MAX_BYTES=64*1024*1024
FORBIDDEN={7878,7879,7880,7881,8090,8092,17881,11434}


def grammar(argv, release):
    """Independent exact argv grammar, not the producer's validation function."""
    root=Path(release['fixture']['root']);uid=release['host_binding']['uid'];token=release['fixture']['token']
    targets={role:f'gui/{uid}/com.codex.reservoir-maintenance-fixture.{token}.{role}' for role in ('provider','reservoir','pin')}
    pid=lambda s:isinstance(s,str) and re.fullmatch('[1-9][0-9]*',s) is not None and int(s)>1
    if argv in (['/usr/sbin/sysctl','-n','kern.bootsessionuuid'],['/usr/bin/sw_vers','-productVersion']):return 'query',None
    if len(argv)==5 and argv[:2]==['/bin/ps','-p'] and pid(argv[2]) and argv[3:]==['-o','lstart=']:return 'query',None
    if len(argv)==6 and argv[:3]==['/bin/ps','-ww','-p'] and pid(argv[3]) and argv[4:]==['-o','command=']:return 'query',None
    if len(argv)==8 and argv[:4]==['/usr/sbin/lsof','-nP','-a','-p'] and pid(argv[4]) and argv[5:]==['-iTCP','-sTCP:LISTEN','-Fpn']:return 'query',None
    if len(argv)==5 and argv[:2]==['/usr/sbin/lsof','-nP'] and argv[2].startswith('-iTCP:') and argv[3:]==['-sTCP:LISTEN','-Fp']:
        port=argv[2][6:];require(re.fullmatch('[1-9][0-9]*',port) is not None and 1024<int(port)<65536 and int(port) not in FORBIDDEN,'native private port differs');return 'query',None
    if len(argv)==3 and argv[:2]==['/bin/launchctl','print'] and argv[2] in targets.values():return 'query',None
    if len(argv)==4 and argv[:3]==['/bin/launchctl','bootstrap',f'gui/{uid}']:
        for role,path in (('provider',root/'provider.plist'),('reservoir',root/'reservoir.plist'),('pin',root/'pin/active.plist')):
            if argv[3]==str(path):return 'bootstrap',role
    if len(argv)==3 and argv[:2]==['/bin/launchctl','bootout'] and argv[2] in targets.values():
        return 'bootout',next(k for k,v in targets.items() if v==argv[2])
    for role in ('provider','reservoir'):
        for hold_token in (token,token+'-cleanup'):
            expected=['/usr/bin/sudo','--','/bin/launchctl','debug',targets[role],'--program',release['python'],'--',
                str(root/'model_environment.py'),'launch','--control-root',str(root/('hold-'+role)),'--hold-only',hold_token]
            if argv==expected:return 'debug',role
    raise ValueError('native argv outside fixed fixture03 grammar')


def query_exit(argv, row):
    rc=row['returncode'];out=row['stdout'];err=row['stderr']
    require(type(rc) is int and isinstance(out,str) and isinstance(err,str),'native query output shape differs')
    if rc==0:return
    if argv[0] in ('/bin/ps','/usr/sbin/lsof'):
        require(rc==1 and not out.strip() and not err.strip(),'native absence is ambiguous');return
    if argv[:2]==['/bin/launchctl','print']:
        require('Could not find service' in out+err or 'Could not find specified service' in out+err,'native job absence is ambiguous');return
    raise ValueError('native query failed')


def inventory(files, section, release):
    root=FIXTURE/'runs'/('owned-native-'+release['fixture']['token'])
    require(section.get('root')==str(root),'native root outside fixed sibling')
    files.directory(root);expected=section.get('artifacts');require(isinstance(expected,dict),'native inventory missing')
    names={'parent',*(s['id'] for s in release['executor_plan'])}
    require(len(names)==72 and {p.name for p in root.iterdir()}==names|{'owner.json'},'native caller tree differs')
    actual={str(root/'owner.json')};total=0
    for name in names:
        directory=root/name;files.directory(directory)
        require({p.name for p in directory.iterdir()}=={'owner.json','commands.jsonl'},'native extra/unresolved artifact')
        actual.update(str(directory/n) for n in ('owner.json','commands.jsonl'))
    require(set(expected)==actual,'native inventory not complete')
    for path,h in expected.items():
        info=Path(path).lstat();total+=info.st_size
        require(stat.S_ISREG(info.st_mode) and info.st_uid==os.getuid() and info.st_nlink==1
                and not info.st_mode & 0o077 and total<=MAX_BYTES and sha_value(h),'unsafe/oversized native tree')
        files.read(path,h)
    return root,names


def check_native(files,fixture,release,executors,parent):
    section=fixture['native'];root,names=inventory(files,section,release)
    require(section['owner']['path']==str(root/'owner.json'),'native root owner path differs')
    owner=files.ref(section['owner']);parent_id=parent['identity'];parent_argv=[release['python'],'-B',str(FIXTURE/'owned_fixture_run.py'),'--execute','--release',fixture['release']['path']]
    require(owner=={'schema':'owned_fixture_native_root_v1','root':str(root),'fixture_root':fixture['root'],
            'token':release['fixture']['token'],'release':fixture['release'],'parent_pid':parent_id['pid'],
            'parent_expected_argv':parent_argv,'python':release['python'],'python_files':release['python_files'],
            'created_wall_time_ns':owner.get('created_wall_time_ns')},'native root ownership differs')
    require(type(owner['created_wall_time_ns']) is int and 0<fixture['started_wall_time_ns']<=owner['created_wall_time_ns']<=fixture['completed_wall_time_ns'], 'native root start differs')
    callers=section['callers'];require([r['caller'] for r in callers]==sorted(names),'native callers missing/reordered')
    commands={};effects=[]
    for item in callers:
        name=item['caller'];base=root/name
        require(item['owner']['path']==str(base/'owner.json') and item['log']['path']==str(base/'commands.jsonl'),'native caller references differ')
        caller=files.ref(item['owner']);executor=None if name=='parent' else executors[name]
        expected=parent_argv if executor is None else executor['launch']['argv'];ident=parent_id if executor is None else executor['identity']
        low=owner['created_wall_time_ns'] if executor is None else executor['launch']['started_wall_time_ns']
        high=fixture['completed_wall_time_ns'] if executor is None else executor['exit']['completed_wall_time_ns']
        require(caller=={'schema':'owned_fixture_native_caller_v1','root_owner':section['owner'],'caller':name,'pid':ident['pid'],
                'expected_argv':expected,'release':fixture['release'],'created_wall_time_ns':caller.get('created_wall_time_ns')}
                and type(caller['created_wall_time_ns']) is int and low<=caller['created_wall_time_ns']<=high
                and process_matches(ident['argv'],expected,release['python_files'],release['python']),'native caller actual identity/time differs')
        data=files.read(item['log']['path'],item['log']['sha256'],content=True,maximum=MAX_BYTES)
        require(not data or data.endswith(b'\n'),'native partial final row');lines=data.splitlines()
        require(len(lines)<=12000 and len(lines)%3==0,'native unresolved command or row bound')
        rows=[];previous=digest(caller);last=caller['created_wall_time_ns'];last_mono=None
        def pairs(items):
            result={}
            for key,value in items:
                require(key not in result,'duplicate native JSON key');result[key]=value
            return result
        for i,line in enumerate(lines,1):
            require(len(line)+1<=262144,'native row oversized');row=json.loads(line,object_pairs_hook=pairs,parse_constant=lambda _:require(False,'nonfinite native JSON'))
            require(isinstance(row,dict) and row.get('schema')=='owned_fixture_native_event_v1' and row.get('sequence')==i
                    and row.get('previous_sha256')==previous and row.get('type')==('intent','created','exit')[(i-1)%3]
                    and row.get('command_id')==(i-1)//3+1 and row.get('caller_owner_sha256')==digest(caller)
                    and type(row.get('wall_time_ns')) is int and last<=row['wall_time_ns']<=high and finite(row.get('monotonic'))
                    and (last_mono is None or last_mono<=row['monotonic']),'native chain/order/time differs')
            common={'schema','sequence','previous_sha256','type','command_id','caller_owner_sha256','wall_time_ns','monotonic'}
            extra={'intent':{'argv','terminal','timeout_s','kind'},'created':{'pid','argv','intent_sha256'},
                   'exit':{'pid','argv','intent_sha256','created_sha256','returncode','naturally_reaped','stdout','stderr','signals_sent'}}[row['type']]
            require(set(row)==common|extra,'native row fields differ')
            previous=digest(row);last=row['wall_time_ns'];last_mono=row['monotonic'];rows.append(row)
        for index in range(0,len(rows),3):
            intent,created,end=rows[index:index+3];argv=intent['argv'];kind,role=grammar(argv,release)
            terminal=kind=='debug' and argv[:2]==['/usr/bin/sudo','--']
            if kind=='debug':require(release.get('sudo_debug') is True and terminal,'debug Terminal policy differs')
            require(intent['kind']==('query' if kind=='query' else 'effect') and intent['terminal'] is terminal
                    and intent['timeout_s']==(180 if terminal else 20),'native intent semantics differ')
            require(created['argv']==end['argv']==argv and created['intent_sha256']==end['intent_sha256']==digest(intent)
                    and end['created_sha256']==digest(created) and type(created['pid']) is int and created['pid']>1
                    and end['pid']==created['pid'] and end['naturally_reaped'] is True and end['signals_sent']==0
                    and type(end['returncode']) is int,'native natural reap/ownership differs')
            if kind=='query':query_exit(argv,end)
            else:
                require(end['returncode']==0 and (not terminal or end['stdout'] is None and end['stderr'] is None),'native effect did not exit cleanly')
                effects.append((name,kind,role))
            link={'path':str(base/'commands.jsonl'),'command_id':intent['command_id'],'intent_sha256':digest(intent),'created_sha256':digest(created),'exit_sha256':digest(end)}
            commands[(link['path'],link['command_id'])]={'link':link,'intent':intent,'created':created,'exit':end,'caller':name}
            if kind=='debug':
                directory=Path(fixture['root'])/('cleanup' if executors[name]['step']['kind']=='cleanup' else '')
                host_rows=[files.json(path) for path in sorted(directory.glob('host-operation-*.json'))]
                starts=[r for r in host_rows if r.get('operation')=='debug_intent' and r.get('role')==role]
                ends=[r for r in host_rows if r.get('operation')=='debug_result' and r.get('role')==role]
                require(len(starts)==len(ends)==1 and starts[0].get('sudo_debug') is release['sudo_debug']
                        and argv==['/usr/bin/sudo','--','/bin/launchctl','debug',release['fixture']['targets'][role],*starts[0]['arguments']]
                        and ends[0].get('native_command')==link
                        and starts[0]['wall_time_ns']<=intent['wall_time_ns']<=end['wall_time_ns']<=ends[0]['wall_time_ns'],
                        'debug native/Host/Terminal evidence differs')
    # Exact finite controls; reconciliation and observation phases cannot replay.
    wanted=[('parent','bootstrap',r) for r in ('provider','reservoir')]
    for name,executor in executors.items():
        step=executor['step'];action=step['action_id'];op,_,role=action.partition(':')
        if step['kind']=='fresh':wanted.append((name,'debug','provider'))
        elif step['mode']=='execute' and op in ('arm_next_spawn','bootstrap_original','remove_held'):
            wanted.append((name,{'arm_next_spawn':'debug','bootstrap_original':'bootstrap','remove_held':'bootout'}[op],role))
        elif step['kind']=='pin' and step['mode']=='execute' and action in ('bootstrap','remove_absent_job'):
            wanted.append((name,'bootstrap' if action=='bootstrap' else 'bootout','pin'))
    require(sorted(effects)==sorted(wanted),'native effect schedule/replay differs')
    # Every exported query/effect row carrying an argv/result must resolve to a
    # real exit, even when it is nested inside an observational witness.
    def visit(value):
        if isinstance(value,dict):
            if {'argv','returncode','stdout','stderr','observed_wall_time_ns'}<=set(value):
                resolve_query(value,commands)
            for child in value.values():visit(child)
        elif isinstance(value,list):
            for child in value:visit(child)
    for path in fixture['artifacts']:
        if path.endswith('.json'):visit(files.json(path))
    return commands


def resolve_query(row,commands,*,caller=None,earliest=None,latest=None):
    link=row.get('native_command');require(isinstance(link,dict),'raw native query lacks command evidence')
    command=commands.get((link.get('path'),link.get('command_id')))
    require(command is not None and link==command['link'],'raw query native chain reference differs')
    end=command['exit'];intent=command['intent'];t=row['observed_wall_time_ns']
    require(all(row.get(k)==end[k] for k in ('argv','returncode','stdout','stderr')) and type(t) is int
            and end['wall_time_ns']<=t and (caller is None or command['caller']==caller)
            and (earliest is None or earliest<=intent['wall_time_ns']) and (latest is None or t<=latest),
            'raw query native timing/caller/output differs')
    return command
