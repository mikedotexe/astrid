"""Read-only retained production-transaction report; never recovery authority.

Import performs no observation. CLI requires explicit paths and --inspect-read-only.
No service requests, numeric imports, signals, administrative commands or writes.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import plistlib
import re
import stat

import production_preflight as p

m = p.m
HOLDER_PYTHON = '/opt/homebrew/Cellar/python@3.14/3.14.3_1/Frameworks/Python.framework/Versions/3.14/bin/python3.14'
HOST_PAIRS = {'hold_intent': 'hold_created', 'debug_intent': 'debug_result',
              'signal_intent': 'signal_sent', 'held_bootout_intent': 'held_job_absent',
              'install_intent': 'install_completed',
              'provider_snapshot_plist_intent': 'provider_snapshot_plist_installed',
              'bootstrap_intent': 'bootstrap_ready', 'hold_release_intent': 'hold_released'}
HOST_CONTROLLER = {'hold_intent': 'create_hold', 'debug_intent': 'arm_next_spawn',
                   'signal_intent': 'signal_stop', 'held_bootout_intent': 'remove_held',
                   'install_intent': 'install_repair', 'provider_snapshot_plist_intent': 'apply_provider_restart',
                   'bootstrap_intent': 'bootstrap_original', 'hold_release_intent': 'release_hold', 'hold_released': 'release_hold'}
OPERATIONS = set(HOST_CONTROLLER.values())
WITNESSES = {'provider_drain','final_snapshot','backup','private_restore','startup','provider_ready'}


def fail(condition, message):
    m.require(condition, message)


def stamp(s):
    return (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_mode, s.st_uid)


class Evidence:
    """Bounded records and streamed immutable bytes; all opens are read-only."""
    def __init__(self):self.files = {}; self.absent_paths = set()

    def absent(self,path):
        path=Path(path);fail(not os.path.lexists(path),'expected absent artifact exists')
        self.absent_paths.add(str(path))

    def read(self, path, *, record=False, allow_alias=False):
        path = Path(path)
        fail(path.is_absolute(), 'artifact path must be absolute')
        target=path.resolve(strict=True);alias=None
        if path!=target:
            fail(allow_alias and not record and path.is_symlink() and path.parent==path.parent.resolve(),
                 'noncanonical/symlink artifact: '+str(path))
            alias={'stat':list(stamp(path.lstat())),'link':str(path.readlink()),'target':str(target)}
        fd = os.open(target, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        try:
            before = os.fstat(fd)
            fail(stat.S_ISREG(before.st_mode) and before.st_nlink == 1, 'special/shared artifact: '+str(path))
            fail(not record or before.st_size <= m.MAX_RECORD, 'oversized JSON record')
            digest = hashlib.sha256(); chunks = []
            while chunk := os.read(fd, 1024*1024):
                digest.update(chunk)
                if record:chunks.append(chunk)
                fail(not record or sum(map(len,chunks)) <= m.MAX_RECORD, 'record grew beyond bound')
            fail(stamp(before) == stamp(os.fstat(fd)) == stamp(target.lstat()), 'artifact changed while read: '+str(path))
            if alias:
                fail(alias=={'stat':list(stamp(path.lstat())),'link':str(path.readlink()),'target':str(path.resolve(strict=True))},
                     'immutable alias changed while read')
            ref = {'path': str(path), 'sha256': digest.hexdigest(), 'stat': list(stamp(before))}
            if alias:ref['immutable_alias']=alias
            previous = self.files.get(str(path))
            fail(previous is None or previous == ref, 'artifact changed across reads: '+str(path))
            self.files[str(path)] = ref
            if not record:return ref['sha256']
            value = json.loads(b''.join(chunks), parse_constant=lambda s: (_ for _ in ()).throw(ValueError('nonfinite JSON')))
            fail(isinstance(value,dict), 'JSON record must be an object')
            return value
        finally:os.close(fd)

    def ref(self, ref):
        fail(isinstance(ref,dict) and m.hash_value(ref.get('sha256')), 'invalid artifact reference')
        fail(self.read(ref['path']) == ref['sha256'], 'artifact reference hash mismatch: '+str(ref['path']))

    def finish(self):
        for path, ref in self.files.items():
            alias=ref.get('immutable_alias');target=Path(alias['target']) if alias else Path(path)
            fail(list(stamp(target.lstat())) == ref['stat'], 'artifact changed during inspection: '+path)
            if alias:
                path_obj=Path(path)
                fail(alias=={'stat':list(stamp(path_obj.lstat())),'link':str(path_obj.readlink()),'target':str(path_obj.resolve(strict=True))},
                     'immutable alias changed during inspection')
        for path in self.absent_paths:fail(not os.path.lexists(path),'absent artifact appeared during inspection: '+path)


def private_directory(path):
    fail(path.is_absolute() and path == path.resolve(strict=True) and path.is_dir(), 'noncanonical transaction directory')
    info = path.stat()
    fail(info.st_uid == os.getuid() and not stat.S_IMODE(info.st_mode)&0o077, 'transaction directory is not private/owned')


def validate_retained(spec, root, evidence):
    """Validate known originals without assuming they are still installed."""
    fail(spec.get('schema') == 'reservoir_production_preparation_v1', 'unsupported preparation')
    plan = m.validate_plan(spec['plan'])
    fail(plan == spec['plan'], 'preparation plan is not normalized')
    fail(root == p.CONTROL_ROOT / plan['transaction_id'] == Path(spec['transaction_root']), 'wrong fixed transaction root')
    fail(set(plan['protected']) == p.PROTECTED and spec['state_root'] == str(p.STATE_ROOT), 'incomplete fixed scope')
    private_directory(root)
    owner = evidence.read(root/'owner.json',record=True)
    fail(owner == {'schema':'reservoir_production_owner_v1','transaction_id':plan['transaction_id'],
                   'actor':plan['actor'],'host_binding':plan['host_binding'],
                   'preparation_sha256':p.preparation_digest(spec)}, 'owner/preparation binding mismatch')
    files = spec['immutable_files']
    fail(isinstance(files,dict) and files and all(Path(path).is_absolute() and m.hash_value(sha) for path,sha in files.items()), 'immutable inventory invalid')
    mutable = {*map(str,p.SOURCES.values()),*map(str,p.PLISTS.values())}
    alias_paths=set(spec['input_groups']['model_snapshot'])|set(spec['input_groups']['interpreter'])
    for path, expected in files.items():
        if path not in mutable:fail(evidence.read(path,allow_alias=path in alias_paths)==expected,'unchanged immutable input differs: '+path)
    for key in ('fixture','runtime_alignment','model_resolution','original_provider_manifest','repair_source','provider_restart_plist'):
        evidence.ref(spec[key])
    fail(spec['repair_source']['sha256']==plan['input_hashes']['reservoir_candidate']
         and spec['provider_restart_plist']['sha256']==plan['input_hashes']['provider_restart_settings'], 'candidate input binding differs')
    variants = {}
    for role in m.ROLES:
        source,settings = p.SOURCES[role],p.PLISTS[role]
        original_source,original_settings = plan['input_hashes'][role+'_source'],plan['input_hashes'][role+'_settings']
        fail(files.get(str(source))==original_source and files.get(str(settings))==original_settings, 'original input hash mismatch')
        for kind,expected in (('source',original_source),('plist',original_settings)):
            saved = Path(spec['originals'][role][kind]);canonical = source if kind=='source' else settings
            fail(saved != canonical and files.get(str(saved))==expected and evidence.read(saved)==expected, 'preserved original differs')
        current_source,current_settings = evidence.read(source),evidence.read(settings)
        source_options = {'original':original_source}
        settings_options = {'original':original_settings}
        if role=='reservoir':source_options['candidate']=plan['input_hashes']['reservoir_candidate']
        if role=='provider':settings_options['pinned']=plan['input_hashes']['provider_restart_settings']
        variants[role] = {'source':next((k for k,v in source_options.items() if current_source==v),'unknown'),
                          'plist':next((k for k,v in settings_options.items() if current_settings==v),'unknown'),
                          'source_sha256':current_source,'plist_sha256':current_settings}
    fail(evidence.read(root/'model_environment.py')==files[str(p.HOLD_SOURCE)], 'copied holder changed')
    return plan, owner, variants


def cursor_shape(cursor, expected):
    fail(cursor.get('path')==str(expected) and m.hash_value(cursor.get('prefix_sha256'))
         and all(type(cursor.get(key)) is int and cursor[key]>=0 for key in ('device','inode','start_offset')),
         'capture anchor cursor is incomplete/wrong path')


def anchors(root, plan, evidence):
    result = {}
    for role in m.ROLES:
        anchor = evidence.read(root/(role+'-capture-anchor.json'),record=True)
        fail(anchor.get('identity')==plan['roles'][role]['identity'], 'capture anchor process identity differs')
        cursor_shape(anchor['log'],p.LOGS[role])
        if role=='provider':
            cursor_shape(anchor['qos'],p.QOS)
            fail(anchor.get('start_worker_idle') is True and anchor.get('start_queue_empty') is True, 'positive provider capture anchor missing')
        else:
            inventory=anchor.get('inventory',{});names=[row.get('name') for row in inventory.get('handles',[])]
            fail(anchor.get('inventory_matches_start_boundary') is True and inventory.get('type')=='list_handles_response'
                 and all(isinstance(name,str) and name for name in names) and len(names)==len(set(names)), 'reservoir capture inventory invalid')
        result[role]=anchor
    return result


def references(value,evidence):
    if isinstance(value,dict):
        if 'path' in value and 'sha256' in value:evidence.ref(value)
        for item in value.values():references(item,evidence)
    elif isinstance(value,list):
        for item in value:references(item,evidence)


def controller_records(journal,plan,evidence):
    private_directory(journal)
    fail(evidence.read(journal/'plan.json',record=True)==plan,'controller plan differs from preparation')
    events=[];previous=m.digest(plan);pending=None;intents={};completed={};receipts={};holders={}
    allowed={'preflight','mutation_intent','mutation_completed','witness','held_verified','job_absent','completed','failed'}
    paths=sorted(journal.glob('event-*.json'));fail(len(paths)<=10000,'too many controller records')
    for index,path in enumerate(paths,1):
        fail(path.name==f'event-{index:05d}.json','controller event sequence incomplete')
        event=evidence.read(path,record=True)
        fail(event.get('sequence')==index and event.get('previous_sha256')==previous and event.get('plan_sha256')==m.digest(plan), 'controller hash chain mismatch')
        previous=m.digest(event);events.append(event);kind=event.get('type');fail(kind in allowed,'unknown controller event')
        if kind in ('mutation_intent','mutation_completed'):
            op,role=event.get('operation'),event.get('role');key=(op,role)
            fail(op in OPERATIONS and (role in m.ROLES if op!='install_repair' else role is None), 'invalid controller operation/role')
            if kind=='mutation_intent':
                fail(pending is None and key not in intents,'overlapping/repeated controller intent')
                pending=key;intents[key]=event
            else:
                fail(pending==key and isinstance(event.get('result'),dict),'controller completion lacks matching intent/result')
                completed[key]=event;pending=None
        elif kind=='witness':
            name=event.get('kind');receipt=event.get('receipt',{})
            fail(name in WITNESSES and name not in receipts,'duplicate/unknown witness')
            fail(receipt.get('schema')=='reservoir_maintenance_witness_v1' and receipt.get('kind')==name
                 and receipt.get('transaction_id')==plan['transaction_id'] and receipt.get('passed') is True,'invalid controller witness')
            expected={'plan_sha256':m.digest(plan),**{r+'_old_identity_sha256':m.digest(plan['roles'][r]['identity']) for r in m.ROLES}}
            fail(all(receipt.get('bindings',{}).get(k)==v for k,v in expected.items()),'witness identity bindings differ')
            fail(evidence.read(journal.parent/(name+'-witness.json'),record=True)==receipt,'standalone witness missing/altered')
            references(receipt,evidence);receipts[name]=receipt
        elif kind=='held_verified':
            role=event.get('role');fail(role in m.ROLES and role not in holders,'duplicate/unknown held role')
            holders[role]=m.identity(event.get('identity'))
        elif kind=='job_absent':fail(event.get('role') in m.ROLES,'unknown absent role')
        elif kind=='completed':
            result=event.get('receipt',{})
            fail(result.get('transaction_id')==plan['transaction_id'] and result.get('plan_sha256')==m.digest(plan)
                 and result.get('witness_hashes')=={k:m.digest(v) for k,v in receipts.items()},'completed result binding differs')
    return {'paths':paths,'events':events,'last_sha256':previous,'pending':pending,'intents':intents,'completed':completed,'witnesses':receipts,'holders':holders}


def host_records(root,plan,controller,evidence, recovery_records=()):
    events=[];pending=None;pairs={};last=-math.inf;standalone={}
    recovery_claims={(r.get('operation'),r.get('role')) for r in recovery_records if r.get('type')=='action_claim'}
    reconciled={r.get('action_id') for r in recovery_records if r.get('type')=='reconciliation_result'}
    recovered_holders={r.get('role'):r.get('result',{}).get('identity') for r in recovery_records
                       if r.get('type') in ('action_result','reconciliation_result') and r.get('operation')=='held_verified'}
    def action_id(pair):
        operation=HOST_CONTROLLER[pair[0]]
        return operation + (':' + pair[1] if pair[1] else '')
    guard={'schema':'model_environment_hold_v1','transaction_id':plan['transaction_id'],'actor':plan['actor']}
    paths=sorted(root.glob('host-operation-*.json'));fail(len(paths)<=10000,'too many host records')
    for index,path in enumerate(paths,1):
        fail(path.name==f'host-operation-{index:05d}.json','host operation sequence incomplete')
        event=evidence.read(path,record=True);op=event.get('operation');role=event.get('role')
        when=event.get('monotonic');fail(type(when) in (int,float) and math.isfinite(when) and when>=last,'host operation clocks invalid')
        last=when;events.append(event)
        if op in ('install_intent','install_completed'):role=None
        elif op in ('provider_snapshot_plist_intent','provider_snapshot_plist_installed'):role='provider'
        else:fail(role in m.ROLES,'host role invalid')
        if op in HOST_PAIRS:
            key=(op,role)
            if pending is not None and action_id(pending) in reconciled: pending=None
            fail(pending is None and key not in pairs,'overlapping/repeated host intent')
            fail((HOST_CONTROLLER[op],role) in controller['intents'] or (HOST_CONTROLLER[op],role) in recovery_claims,
                 'host operation lacks controller or recovery execution claim')
            pending=key;pairs[key]={'intent':event,'result':None}
        elif op=='bootstrap_command_returned':
            fail(pending==('bootstrap_intent',role) and 'command_returned' not in pairs[pending],
                 'bootstrap command result lacks unique pending intent')
            fail(type(event.get('returncode')) is int and type(event.get('wall_time_ns')) is int
                 and event['wall_time_ns']>=pairs[pending]['intent'].get('wall_time_ns',-1),
                 'bootstrap return boundary incomplete')
            pairs[pending]['command_returned']=event
        elif op=='hold_released' and pending!=('hold_release_intent',role):
            fail((('release_hold',role) in controller['intents'] or ('release_hold',role) in recovery_claims)
                 and role not in standalone and event.get('guard')==guard,'hold release record invalid')
            standalone[role]=event
        elif op in HOST_PAIRS.values():
            fail(pending is not None and op==HOST_PAIRS[pending[0]] and role==pending[1],'host result lacks matching intent')
            pairs[pending]['result']=event
            if pending[0]=='hold_release_intent':standalone[role]=event
            pending=None
        elif op=='hold_released':
            fail(('release_hold',role) in controller['intents'] and role not in standalone and event.get('guard')==guard,'hold release record invalid')
            standalone[role]=event
        else:raise m.Refusal('unknown host operation')
    for (kind,role),pair in pairs.items():
        event,result=pair['intent'],pair['result'];old=plan['roles'][role]['identity'] if role else None
        if kind=='hold_intent':fail(event.get('guard')==guard and (result is None or result.get('guard')==guard),'host hold owner differs')
        elif kind=='debug_intent':
            expected=['/usr/bin/sudo','--','/bin/launchctl','debug',f"gui/{plan['host_binding']['uid']}/{m.LABELS[role]}",
                      '--program',HOLDER_PYTHON,'--',str(root/'model_environment.py'),'launch','--control-root',str(root/('hold-'+role)),
                      '--hold-only',plan['transaction_id']]
            fail(event.get('identity')==old and event.get('command')==expected,'debug target/identity/argv differs')
            fail(result is None or type(result.get('returncode')) is int,'debug result incomplete')
        elif kind=='signal_intent':
            fail(event.get('identity')==old and event.get('signal')=='SIGTERM' and (result is None or result.get('identity')==old),'cooperative signal identity differs')
        elif kind=='held_bootout_intent':
            held=controller['holders'].get(role) or recovered_holders.get(role)
            fail(held is not None and event.get('identity')==held and (result is None or result.get('identity')==held),'holder removal identity differs')
        elif kind=='install_intent':
            fail(event.get('target')==str(p.SOURCES['reservoir']) and event.get('source_sha256')==plan['input_hashes']['reservoir_candidate']
                 and (result is None or result.get('source_sha256')==event['source_sha256']),'repair install binding differs')
        elif kind=='provider_snapshot_plist_intent':
            fail(event.get('original_sha256')==plan['input_hashes']['provider_settings']
                 and event.get('restart_sha256')==plan['input_hashes']['provider_restart_settings']
                 and (result is None or result.get('restart_sha256')==event['restart_sha256']),'pinned plist binding differs')
        elif kind=='hold_release_intent':
            fail(event.get('guard')==guard and (result is None or result.get('guard')==guard),'hold release guard differs')
        elif kind=='bootstrap_intent':
            expected=plan['input_hashes']['provider_restart_settings' if role=='provider' else 'reservoir_settings']
            fail(event.get('applied_plist_sha256')==expected and event.get('original_rollback_plist_sha256')==plan['input_hashes'][role+'_settings'],'bootstrap plist binding differs')
            cursor_shape(event['log_anchor'],p.LOGS[role])
            if result is not None:
                m.identity(result.get('identity'))
                fail('command_returned' not in pair or pair['command_returned']['returncode']==0,
                     'bootstrap readiness contradicts recorded command failure')
    mapping={'create_hold':'hold_intent','arm_next_spawn':'debug_intent','signal_stop':'signal_intent',
             'remove_held':'held_bootout_intent','install_repair':'install_intent',
             'apply_provider_restart':'provider_snapshot_plist_intent','bootstrap_original':'bootstrap_intent'}
    completion_rows=[(op,role,event['result'],None) for (op,role),event in controller['completed'].items()]
    completion_rows.extend((row.get('operation'),row.get('role'),row.get('result'),row)
                           for row in recovery_records if row.get('type')=='action_result'
                           and row.get('operation') in OPERATIONS)
    for op,role,result,recovery in completion_rows:
        if op=='release_hold':
            fail(role in standalone and result.get('released') is True,'completed hold release lacks host result')
            if recovery:
                span=recovery.get('host_operations',{})
                fail(span.get('before_count',len(events)) < events.index(standalone[role])+1 <= span.get('after_count',-1),
                     'recovery release lacks its own Host result')
            continue
        pair=pairs.get((mapping[op],role));fail(pair is not None and pair['result'] is not None,'completed controller operation lacks host result')
        host=pair['result']
        if recovery:
            span=recovery.get('host_operations',{})
            fail(span.get('before_count',len(events)) < events.index(pair['intent'])+1
                 <= events.index(host)+1 <= span.get('after_count',-1),
                 'recovery mutation lacks its own Host intent/result span')
        checks={'create_hold':result.get('guard')==guard,
                'arm_next_spawn':result.get('armed') is True and host.get('returncode')==0,
                'signal_stop':result.get('signal')=='SIGTERM' and result.get('identity')==host.get('identity') and result.get('force_used') is False,
                'remove_held':result.get('removed_identity')==host.get('identity'),
                'install_repair':result.get('source_sha256')==host.get('source_sha256'),
                'apply_provider_restart':role=='provider' and result.get('provider_settings_sha256')==host.get('restart_sha256')
                    and isinstance(result.get('provider_restart_argv'),list),
                'bootstrap_original':result.get('identity')==host.get('identity')}
        fail(checks[op],'controller/host result mismatch: '+op)
    return {'paths':paths,'events':events,'pending':pending,'pairs':pairs,'releases':standalone}


def restore_execution(root,spec,plan,controller,evidence,unresolved):
    """Observe the finite runner's exclusive records, never a child or service.

    No missing result is interpreted as permission to launch another child.
    Even a complete child exit is not the outer accepted private_restore witness.
    """
    names={'launch':'private-restore-launch.json','start':'private-restore-child-start.json',
           'exit':'private-restore-child-exit.json','unresolved':'private-restore-unresolved.json'}
    schemas={'launch':'launch','start':'child_start','exit':'child_exit','unresolved':'unresolved'}
    records={}
    for key,name in names.items():
        path=root/name
        if os.path.lexists(path):records[key]=evidence.read(path,record=True)
        else:evidence.absent(path)
    request_path=root/'private-restore-request.json'
    request=evidence.read(request_path,record=True) if os.path.lexists(request_path) else None
    if request is None:evidence.absent(request_path)
    if not records:
        fail('private_restore' not in controller['witnesses'],'outer restore witness lacks child launch/exit evidence')
        if request is not None:unresolved.append('private restore request exists without launch result; no relaunch inferred')
        return {'status':'request_only' if request is not None else 'not_started','records':records}
    fail('launch' in records and request is not None,'restore child records lack launch/request')
    launch=records['launch'];backup=controller['witnesses'].get('backup')
    fail(backup is not None,'restore launch has no accepted backup witness')
    fail(request.get('schema')=='reservoir_private_restore_request_v1' and request.get('passed') is False
         and request.get('context',{}).get('plan')==plan and request.get('context',{}).get('transaction_id')==plan['transaction_id']
         and request.get('backup_witness')==backup,'restore request belongs to another transaction/backup')
    argv=[str(p.WORKSPACE/'.venv/bin/python'),'-B',str(p.HERE/'restore_verifier.py'),
          '--request',str(request_path),'--execute-private-restore']
    binding={'transaction_id':plan['transaction_id'],'plan_sha256':m.digest(plan),
             'preparation_sha256':p.preparation_digest(spec),
             'request':{'path':str(request_path),'sha256':evidence.files[str(request_path)]['sha256']},
             'backup_sha256':m.digest(backup),'work_root':str(p.HERE/'restore-runs'/('production-'+plan['transaction_id'])),
             'argv':argv,'implementation':launch.get('implementation')}
    implementation=binding['implementation']
    expected_helpers={str(p.HERE/'restore_verifier.py'),str(p.HERE/'reservoir_maintenance_state.py'),str(p.HOLD_SOURCE)}
    fail(isinstance(implementation,dict) and set(implementation)==expected_helpers,'restore implementation closure differs')
    for path,sha in implementation.items():
        fail(spec['immutable_files'].get(path)==sha and evidence.read(path)==sha,'restore implementation changed/unbound')
    for key,record in records.items():
        fail(record.get('schema')=='reservoir_production_restore_'+schemas[key]+'_v1'
             and all(record.get(k)==v for k,v in binding.items()),'restore child binding differs: '+key)
    fail(launch.get('automatic_retry_permitted') is False and launch.get('timeout_s')==300,'restore launch policy differs')
    start_wall=launch.get('started_wall')
    fail(type(start_wall) in (int,float) and math.isfinite(start_wall) and start_wall>0,'restore launch time invalid')
    if 'start' in records:
        fail(type(records['start'].get('pid')) is int and records['start']['pid']>1
             and records['start'].get('signals_permitted') is False
             and type(records['start'].get('started_wall')) in (int,float)
             and math.isfinite(records['start']['started_wall']) and records['start']['started_wall']>=start_wall,'restore child start invalid')
    if 'exit' in records:
        exit_record=records['exit']
        fail('start' in records and exit_record.get('pid')==records['start']['pid']
             and type(exit_record.get('returncode')) is int and exit_record.get('reaped') is True
             and type(exit_record.get('completed_wall')) in (int,float) and math.isfinite(exit_record['completed_wall'])
             and exit_record['completed_wall']>=records['start']['started_wall'],'restore child exit invalid')
        for stream in ('stdout','stderr'):
            fail(exit_record.get(stream,{}).get('path')==str(root/('private-restore-'+stream+'.txt')),'restore log path differs')
            evidence.ref(exit_record[stream])
    if 'unresolved' in records:
        row=records['unresolved'];pid=row.get('pid')
        fail((pid is None or type(pid) is int and pid>1)
             and ('start' not in records or pid==records['start']['pid'])
             and all(row.get(k) is False for k in ('automatic_retry_permitted','next_child_permitted','force_used','production_install_permitted')),
             'restore unresolved record differs')
    accepted='private_restore' in controller['witnesses']
    clean_exit='exit' in records and records['exit']['returncode']==0 and 'unresolved' not in records
    if accepted:fail(clean_exit,'accepted outer restore witness lacks a clean recorded child exit')
    if not clean_exit:
        unresolved.append('private restore launch/child outcome unresolved or failed; never relaunch or install')
    elif not accepted:
        unresolved.append('private restore child exited but outer accepted witness is missing; no inferred success')
    return {'status':'outer_witness_recorded' if accepted else 'exited_without_outer_witness' if clean_exit else 'unresolved',
            'records':records,'outer_witness_recorded':accepted,
            'child_running_now':'not observed; no process lookup or retry authority'}


class ReadOnlyIO(p.ReadOnlySystem):
    """An allowlisted process/launchd observer. Service requests are forbidden."""
    def command(self,*argv):
        allowed=(len(argv)==5 and argv[:3]==('/bin/ps','-o','lstart=') and argv[3]=='-p' and str(argv[4]).isdigit()
                 or len(argv)==6 and argv[:4]==('/bin/ps','-ww','-o','command=') and argv[4]=='-p' and str(argv[5]).isdigit()
                 or len(argv)==3 and argv[:2]==('/bin/launchctl','print') and argv[2] in {f'gui/{os.getuid()}/{v}' for v in m.LABELS.values()}
                 or argv==('/usr/sbin/sysctl','-n','kern.bootsessionuuid') or argv==('/usr/bin/sw_vers','-productVersion')
                 or len(argv)==5 and argv[0:2]==('/usr/sbin/lsof','-nP') and argv[2] in {f'-iTCP:{port}' for port in m.PORTS.values()} and argv[3:]==('-sTCP:LISTEN','-Fp')
                 or len(argv)==8 and argv[:4]==('/usr/sbin/lsof','-nP','-a','-p') and str(argv[4]).isdigit() and argv[5:]==('-iTCP','-sTCP:LISTEN','-Fp'))
        fail(allowed,'non-read-only or nonfixed command rejected')
        return super().command(*argv)
    def health(self):raise m.Refusal('service probes forbidden in recovery inspection')
    def list_handles(self):raise m.Refusal('service probes forbidden in recovery inspection')
    def all_listeners(self,pid):
        result=self.command('/usr/sbin/lsof','-nP','-a','-p',str(pid),'-iTCP','-sTCP:LISTEN','-Fp')
        fail(result.returncode in (0,1) and not result.stderr.strip(),'holder listener inspection failed')
        return sorted({int(line[1:]) for line in result.stdout.splitlines() if line.startswith('p')})


def snapshot(io,plan):
    return {'host':io.host(),'stewardship':io.stewardship(),
            'protected':{name:io.process(old['pid']) for name,old in plan['protected'].items()},
            'roles':{role:io.role(role) for role in m.ROLES},
            'old_pids':{role:io.process(plan['roles'][role]['identity']['pid']) for role in m.ROLES}}


def classify_roles(spec,root,plan,view,controller,host,evidence,unresolved,recovery_records=()):
    guard={'schema':'model_environment_hold_v1','transaction_id':plan['transaction_id'],'actor':plan['actor']}
    fixture=evidence.read(spec['fixture']['path'],record=True)
    aliases=set(fixture.get('python_executables',{}))
    fail(HOLDER_PYTHON in aliases and all(spec['immutable_files'].get(path)==sha for path,sha in fixture['python_executables'].items()),
         'qualified holder runtime not bound in preparation')
    recovered={row.get('action_id'):row.get('result') for row in recovery_records
               if row.get('type') in ('action_result','reconciliation_result')}
    result={}
    for role in m.ROLES:
        observed=view['roles'][role];process=observed.get('process');control=root/('hold-'+role)
        fail(observed.get('target')==f"gui/{plan['host_binding']['uid']}/{m.LABELS[role]}"
             and type(observed.get('job_loaded')) is bool and isinstance(observed.get('listener_pids'),list),'invalid fixed role observation')
        hold_path=control/'hold.json';held_guard=evidence.read(hold_path,record=True) if os.path.lexists(hold_path) else None
        if held_guard is None:evidence.absent(hold_path)
        fail(held_guard is None or held_guard==guard,'foreign/changed hold guard')
        row={'observed':observed,'guard':held_guard,'classification':'unknown','old_pid_observation':view['old_pids'][role]}
        if process is None:
            row['classification']='absent_observed' if not observed['job_loaded'] and not observed['listener_pids'] else 'pending_or_inconsistent_no_process'
        else:
            m.identity(process);old=plan['roles'][role]['identity'];argv=process['argv']
            expected_hold=[HOLDER_PYTHON,str(root/'model_environment.py'),'launch','--control-root',str(control),'--hold-only',plan['transaction_id']]
            holder_command=argv[1:]==expected_hold[1:] and argv[0] in {HOLDER_PYTHON,*aliases}
            if process==old:row['classification']='original_identity_observed'
            elif holder_command:
                path=control/'held'/(str(process['pid'])+'.json')
                if not path.exists():
                    evidence.absent(path);row['classification']='holder_receipt_pending_or_missing'
                else:
                    receipt=evidence.read(path,record=True)
                    fail(held_guard==guard and receipt.get('schema')=='model_environment_held_v1' and receipt.get('hold')==guard
                         and receipt.get('pid')==process['pid'] and receipt.get('started_at')==process['started_at']
                         and receipt.get('hold_only') is True and receipt.get('model_imported') is False
                         and receipt.get('listener_opened') is False and receipt.get('launcher_sha256')==evidence.read(root/'model_environment.py'), 'holder receipt does not match exact owned process')
                    row['classification']='owned_holder_observed';row['held_receipt']=receipt
            else:
                pair=host['pairs'].get(('bootstrap_intent',role));recorded=(pair['result'].get('identity') if pair and pair['result'] else
                    recovered.get('bootstrap_original:'+role,{}).get('identity'))
                expected=spec['provider_restart_argv'] if role=='provider' else old['argv']
                if process==recorded and argv==expected:row['classification']='recorded_replacement_observed'
                elif pair and argv==expected and process!=old:
                    row['classification']='unrecorded_replacement_candidate'
                    row['bootstrap_intent']=pair['intent']
                    row['bootstrap_command_returned']=pair.get('command_returned')
                    unresolved.append(role+': exact replacement requires phase-specific birth/readiness/startup reconciliation')
                else:row['classification']='unknown_process'
        if row['classification'] in ('owned_holder_observed','absent_observed','recorded_replacement_observed','unrecorded_replacement_candidate') and view['old_pids'][role]==plan['roles'][role]['identity']:
            unresolved.append(role+': original process survives outside expected job state')
        if process is not None and observed['job_loaded'] is not True:
            unresolved.append(role+': process reported without a loaded fixed job')
        if row['classification']=='owned_holder_observed' and observed['listener_pids']:
            unresolved.append(role+': holder unexpectedly owns/overlaps production listener')
        if process and row['classification'] in ('original_identity_observed','recorded_replacement_observed','unrecorded_replacement_candidate') and observed['listener_pids']!=[process['pid']]:
            unresolved.append(role+': running identity listener differs or is not yet ready')
        if row['classification'] in ('unknown_process','pending_or_inconsistent_no_process','holder_receipt_pending_or_missing'):
            unresolved.append(role+': '+row['classification'])
        if ('create_hold',role) in controller['completed'] and ('release_hold',role) not in controller['completed'] and 'release_hold:'+role not in recovered and held_guard is None:
            unresolved.append(role+': recorded hold unexpectedly absent or release outcome unrecorded')
        result[role]=row
    return result





def validate_recovery_spans(root,events,records,evidence):
    """Bind later Host files to their own one-step recovery claims/results."""
    claims={}
    for row in records:
        kind=row['type']
        if kind in ('action_claim','reconciliation_claim'):
            count=row.get('host_before_count')
            fail(type(count) is int and 0<=count<=len(events)
                 and row.get('host_before_sha256')==m.digest(events[:count]),'recovery claim Host prefix differs')
            claims[m.digest(row)]=row
        elif kind in ('action_result','action_failed','reconciliation_result'):
            claim=claims.get(row.get('claim_sha256'));span=row.get('host_operations',{})
            fail(claim is not None,'recovery result lacks claimed Host prefix')
            if span.get('unreadable') is True:
                fail(kind=='action_failed','unreadable success Host span');continue
            first,last=span.get('before_count'),span.get('after_count')
            fail(type(first) is int and type(last) is int and first==claim['host_before_count']
                 and 0<=first<=last<=len(events) and span.get('before_sha256')==m.digest(events[:first])
                 and span.get('after_sha256')==m.digest(events[:last]),'recovery Host result span differs')
            refs=span.get('appended',[]);fail(len(refs)==last-first,'recovery appended Host set differs')
            for number,ref in enumerate(refs,first+1):
                fail(ref.get('path')==str(root/f'host-operation-{number:05d}.json')
                     and ref.get('record_sha256')==m.digest(events[number-1]),'recovery Host record differs')
                evidence.ref(ref)
            if kind=='reconciliation_result':fail(first==last,'reconciliation replayed a Host operation')


def output_presence(root,plan):
    paths={'provider_drain':['provider-drain.log','provider-drain-qos.jsonl','final-coupling-journal.json'],
           'final_snapshot':['reservoir-drain.log'], 'backup':['stopped-backup'],
           'private_restore':['private-restore-request.json','private-restore-launch.json',
               'private-restore-child-start.json','private-restore-child-exit.json','private-restore-unresolved.json'],
           'startup':['reservoir-startup.log'], 'provider_ready':[]}
    result={kind:any(os.path.lexists(root/name) for name in [*names,kind+'-witness.json'])
            for kind,names in paths.items()}
    result['private_restore'] |= os.path.lexists(p.HERE/'restore-runs'/('production-'+plan['transaction_id']))
    return result


def retained_recovery(root,plan,spec,controller,evidence):
    """Validate separate recovery authority without rewriting original events."""
    from recovery_ledger import read_ledger
    ledger=read_ledger(root,plan)
    header=ledger['header']
    if header is not None:
        fail(header.get('preparation_sha256')==p.preparation_digest(spec)
             and header.get('original_controller_head_sha256')==controller['last_sha256'],
             'recovery ledger belongs to another original journal/preparation')
        for path in sorted((root/'recovery').glob('*.json')):evidence.read(path,record=True)
        for row in ledger['records']:references(row,evidence)
    else:evidence.absent(root/'recovery')
    return ledger


def inspect(preparation_path,journal_path,*,io):
    """No writes; every live observation requires an explicitly supplied IO."""
    report={'schema':'reservoir_production_recovery_inspection_v1','evidence_valid':False,
            'automatic_resume_allowed':False,'retry_authorized':False,'production_recovery_qualified':False,
            'controller_enabled':False,'mutation_performed':False,'service_probes_performed':False,
            'unresolved':[],'errors':[]}
    evidence=Evidence()
    try:
        spec=evidence.read(Path(preparation_path),record=True);root=Path(spec['transaction_root'])
        plan,owner,variants=validate_retained(spec,root,evidence)
        journal=Path(journal_path)
        fail(journal==root/'controller','controller journal must be the fixed transaction_root/controller child')
        before=snapshot(io,plan)
        fail(before['host']==plan['host_binding'],'current host differs')
        if before['stewardship']!={'paused':True,'pause_generation':plan['pause_generation'],'pause_actor':plan['actor'],'lease':None}:
            report['unresolved'].append('current stewardship differs; no ownership authority inferred')
        if before['protected']!=plan['protected']:report['unresolved'].append('one or more protected process identities changed')
        report.update(transaction_id=plan['transaction_id'],preparation_sha256=p.preparation_digest(spec),owner=owner,
                      installed_variants=variants,capture_anchors=anchors(root,plan,evidence))
        for role,variant in variants.items():
            if 'unknown' in (variant['source'],variant['plist']):
                report['unresolved'].append(role+': installed source/plist is outside preserved original and reviewed replacement')
        controller=controller_records(journal,plan,evidence)
        recovery=retained_recovery(root,plan,spec,controller,evidence)
        host=host_records(root,plan,controller,evidence,recovery['records'])
        report['recovery_ledger']=recovery
        resolved={row.get('action_id') for row in recovery['records'] if row.get('type') in ('action_result','reconciliation_result')}
        report['historical_unresolved']=[]
        report['recovery_output_presence']=output_presence(root,plan)
        pin=controller['completed'].get(('apply_provider_restart','provider'))
        if pin is not None:
            fail(pin['result'].get('provider_restart_argv')==spec['provider_restart_argv'],
                 'provider restart completion argv differs from preparation')
        effective=dict(controller,witnesses=dict(controller['witnesses']))
        for row in recovery['records']:
            if row.get('type') in ('action_result','reconciliation_result') and row.get('action_id','').startswith('witness:'):
                kind=row.get('action_id','').split(':')[-1]
                receipt=row.get('result',{})
                fail(kind in WITNESSES and receipt.get('kind')==kind and receipt.get('passed') is True,
                     'recovery witness result invalid')
                fail(evidence.read(root/(kind+'-witness.json'),record=True)==receipt,'recovery standalone witness differs')
                references(receipt,evidence);effective['witnesses'][kind]=receipt
        report['private_restore_execution']=restore_execution(root,spec,plan,effective,evidence,report['unresolved'])
        validate_recovery_spans(root,host['events'],recovery['records'],evidence)
        boundaries={}
        for (kind,role),pair in host['pairs'].items():
            if kind=='signal_intent':
                boundary=pair['intent'].get('wall_time_ns')
                if type(boundary) is not int or boundary<=0:
                    report['unresolved'].append(role+': durable signal wall-time boundary unavailable; final-file timestamp gate cannot be reconstructed')
                else:boundaries[role]=boundary
        report['signal_wall_time_ns']=boundaries
        report['bootstrap_wall_time_ns']={}
        for (kind,role),pair in host['pairs'].items():
            if kind=='bootstrap_intent':
                boundary=pair['intent'].get('wall_time_ns')
                if type(boundary) is int and boundary>0:report['bootstrap_wall_time_ns'][role]=boundary
                else:report['unresolved'].append(role+': bootstrap birth boundary unavailable; never infer original launch outcome')
        for label,pending in (('controller',controller['pending']),('host',host['pending'])):
            if pending:
                op=(HOST_CONTROLLER[pending[0]] if label=='host' else pending[0])
                action_id=op+(':'+pending[1] if pending[1] else '')
                report['historical_unresolved' if action_id in resolved else 'unresolved'].append(
                    label+' intent without original confirmed result: '+str(pending))
        for key,pair in host['pairs'].items():
            if key[0]=='debug_intent' and pair['result'] is not None and pair['result'].get('returncode')!=0:
                report['historical_unresolved' if 'arm_next_spawn:'+key[1] in resolved else 'unresolved'].append('debug invocation nonzero; override state unproven: '+key[1])
        report['roles']=classify_roles(spec,root,plan,before,controller,host,evidence,report['unresolved'],recovery['records'])
        for role,row in report['roles'].items():
            if row['classification']=='original_identity_observed' and variants[role] != {
                    'source':'original','plist':'original',
                    'source_sha256':plan['input_hashes'][role+'_source'],
                    'plist_sha256':plan['input_hashes'][role+'_settings']}:
                report['unresolved'].append(role+': old process survives with changed on-disk source/plist; loaded bytes not inferred')
            if row['classification'] in ('recorded_replacement_observed','unrecorded_replacement_candidate'):
                expected_source='candidate' if role=='reservoir' else 'original'
                expected_plist='pinned' if role=='provider' else 'original'
                if variants[role]['source']!=expected_source or variants[role]['plist']!=expected_plist:
                    report['unresolved'].append(role+': recorded replacement differs from expected installed source/plist')
            if row['classification']=='owned_holder_observed':
                pair=host['pairs'].get(('debug_intent',role))
                if pair is None or pair['result'] is None:
                    report['historical_unresolved' if 'arm_next_spawn:'+role in resolved else 'unresolved'].append(role+': holder observation does not resolve missing debug result')
            if row['classification']=='owned_holder_observed':
                listeners=io.all_listeners(row['observed']['process']['pid']);row['all_tcp_listener_pids']=listeners
                if listeners:report['unresolved'].append(role+': holder has TCP listeners')
        after=snapshot(io,plan)
        if before!=after:report['unresolved'].append('live inspection changed between before/after snapshots; observation inconclusive')
        report.update(observed_before=before,observed_after=after,
                      controller={'events':controller['events'],'last_sha256':controller['last_sha256'],'pending':controller['pending']},
                      host_operations={'events':host['events'],'pending':host['pending'],'chain_limit':'Ordered contiguous files, not an independent authenticated hash chain; cross-checked against controller intents/results.'},
                      possibly_pending_debug_roles=sorted({role for kind,role in host['pairs'] if kind=='debug_intent'
                        and not (host['pairs'].get(('held_bootout_intent',role),{}).get('result') is not None
                                 and ('remove_held',role) in controller['completed'])}))
        fail(sorted(journal.glob('event-*.json'))==controller['paths']
             and sorted(root.glob('host-operation-*.json'))==host['paths'], 'retained record set changed during inspection')
        fail(output_presence(root,plan)==report['recovery_output_presence'],'phase output presence changed during inspection')
        fail(retained_recovery(root,plan,spec,controller,evidence)==recovery,'recovery ledger changed during inspection')
        evidence.finish();report['evidence_valid']=True
    except (Exception,KeyboardInterrupt) as error:
        report['errors'].append(type(error).__name__+': '+str(error))
        report['unresolved'].append('retained evidence or observation incomplete; do not infer any completed operation')
    report['artifacts']=evidence.files
    report['next_action']='Retain existing state, guards and receipts. Human-reviewed reconciliation of listed identities and unresolved intents is required; this report authorizes no retry, signal, override, bootstrap, hold release, rollback or continuation.'
    report['limitations']=['Inspector alone does not qualify production recovery or enable the controller.',
        'No live health, reservoir state or numeric semantics were probed; file bytes do not prove loaded source.',
        'Capture anchors are checked as retained records; live log prefixes and private-restore semantics are not requalified.',
        'Hash chains detect mismatched records, not a fully rewritten self-consistent history or missing unanchored tail.',
        'An intent without its recorded result remains unresolved even when current state appears consistent with success.']
    return report


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--preparation',type=Path,required=True)
    parser.add_argument('--journal',type=Path,required=True)
    parser.add_argument('--inspect-read-only',action='store_true',required=True)
    args=parser.parse_args()
    result=inspect(args.preparation,args.journal,io=ReadOnlyIO())
    print(json.dumps(result,indent=2,allow_nan=False))
    return 0 if result['evidence_valid'] else 1

if __name__=='__main__':raise SystemExit(main())
