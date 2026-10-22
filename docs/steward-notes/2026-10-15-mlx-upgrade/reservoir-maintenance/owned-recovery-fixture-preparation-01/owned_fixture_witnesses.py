"""Finite synthetic-worker evidence. No numeric or production persistence claim."""
from __future__ import annotations
import copy
import hashlib
import json
from pathlib import Path

import owned_fixture_records as r

KINDS=('provider_drain','final_snapshot','backup','private_restore','startup','provider_ready')
STATE={'fixture_astrid':{'ticks':11},'fixture_minime':{'ticks':7}}


def context_for(host,kind=None):
    phases=host.core['recovery_phases']
    state={'execution_scope':host.scope,'receipts':host.receipts,
           'replacements':{role:value['identity'] for role,value in host.startups.items() if 'identity' in value}}
    return phases.context(host.plan,state,{'witness':kind})


def prefix(anchor):
    path=Path(anchor['path']);data=r.raw(path);info=path.stat();end=anchor['start_offset']
    r.require((info.st_dev,info.st_ino)==(anchor['device'],anchor['inode'])
              and len(data)>=end and hashlib.sha256(data[:end]).hexdigest()==anchor['prefix_sha256'],
              'original worker log prefix changed')
    return data[end:]


def phases(rows,expected):
    names=[row['phase'] for row in rows]
    r.require(names==expected,'worker event order differs: '+repr(names))
    times=[row.get('monotonic') for row in rows]
    r.require(all(type(value) in (int,float) for value in times) and times==sorted(times),
              'worker event timestamps differ')


def drain(host,role,identity,expected,*,provider_identity=None,reservoir_identity=None,wait_applied=False):
    r.require(host.io.process(identity['pid']) is None,'drained exact worker still exists or PID was reused')
    rows=host.events(role,identity['pid'],complete=True)
    if role=='provider':
        phases(rows,['started','idle','generation_admitted','stop_received','checkin_sent','checkin_acknowledged','drained'])
        request_id=rows[2]['request_id']
        r.require(all(row.get('request_id')==request_id for row in rows[2:])
                  and rows[2]['ticks']==rows[5]['ticks']==11
                  and rows[4]['monotonic']-rows[3]['monotonic']>=5.9,'provider drain/check-in timing differs')
        request_path=host.root/('request.'+request_id+'.json');ack_path=host.root/('ack.'+request_id+'.json')
        request=r.read(request_path);ack=r.read(ack_path)
        r.require(request=={'token':host.token,'request_id':request_id,'handle':'fixture_astrid',
                  'ticks':11,'provider_pid':identity['pid']}
                  and ack=={'token':host.token,'request_id':request_id,'ticks':11,'ok':True},'check-in request/ACK differs')
        def observed_application():
            applied=[row for row in host.events('reservoir') if row['phase']=='checkin_applied' and row['request_id']==request_id]
            r.require(len(applied)<=1,'duplicate exact check-in application')
            if not applied:return None
            r.require(applied[0]['state']==expected and (reservoir_identity is None or applied[0]['pid']==reservoir_identity['pid']),
                      'exact check-in applied by wrong worker or with wrong state')
            return applied[0]
        if wait_applied:
            r.identity(reservoir_identity)
            def guarded_application():
                view=host.setup_observe('reservoir')
                r.require(view['process']==reservoir_identity and view.get('ready') is True
                          and view['listener_pids']==[reservoir_identity['pid']]
                          and host.io.process(identity['pid']) is None
                          and r.read(request_path)==request and r.read(ack_path)==ack,
                          'check-in observation identity/request/ACK changed')
                return observed_application()
            applied=host.wait(guarded_application,'exact reservoir check-in log publication',30)
        else:
            applied=observed_application()
            r.require(applied is not None,'reservoir exact check-in application event is absent')
        return {'identity':identity,'events':rows,'request':r.reference(request_path),'ack':r.reference(ack_path),
                'request_id':request_id,'applied_event':applied,'expected_state':expected,'drain_exceeds_five_seconds':True}
    phases(rows,['started','restored','checkin_applied','stop_received','final_snapshot_started',
                 'handle_saved','handle_saved','final_snapshot_complete','drained'])
    r.require(rows[4]['monotonic']-rows[3]['monotonic']>=5.9
              and rows[4]['handles']==sorted(expected) and rows[7]['state']==expected
              and {row['name']:row['value'] for row in rows[5:7]}==expected
              and r.read(host.root/'state.json')==expected,'final saved fixture state differs')
    if provider_identity:
        acked=drain(host,'provider',provider_identity,expected)
        r.require(rows[2]['request_id']==acked['request_id'],'final save did not include final provider check-in')
    return {'identity':identity,'events':rows,'state':r.reference(host.root/'state.json'),
            'expected_state':expected,'final_handles':sorted(expected),'drain_exceeds_five_seconds':True}


def ready(host,role,identity):
    view=host.setup_observe(role)
    r.require(view['process']==identity and view.get('ready') is True and not view.get('held')
              and view['listener_pids']==[identity['pid']],'same-PID replacement readiness differs')
    rows=host.events(role,identity['pid'])
    phases(rows,['started','idle'] if role=='provider' else ['started','restored'])
    if role=='reservoir':r.require(rows[1]['state']==STATE,'replacement restored state differs')
    return {'identity':identity,'events':rows,'sentinel_port':host.ports[role],'ready':True}


def preserve(path,value,produce):
    if produce:r.write(path,value)
    else:r.require(r.read(path)==value,'retained fixture evidence differs')
    return r.reference(path)


def backup_payload(host):
    files={}
    for path in sorted([host.root/'state.json',*host.root.glob('request.*.json'),*host.root.glob('ack.*.json')]):
        files[path.name]={'sha256':r.reference(path)['sha256'],'value':r.read(path)}
    r.require(files['state.json']['value']==STATE and len(files)==3,'unexpected stopped fixture inventory')
    return {'scope':'owned_fixture_only','files':files,'expected_state':STATE,'all_synthetic_history_included':True}


def make_witness(host,kind,context,*,produce=False):
    r.require(kind in KINDS and context==context_for(host,kind),'fixture witness context differs')
    old={role:host.plan['roles'][role]['identity'] for role in ('provider','reservoir')}
    evidence={'scope':'owned_fixture_only','production_qualified':False,'numeric_qualification':False}
    outer=host.root/(kind+'-witness.json')
    if kind in ('provider_drain','final_snapshot'):
        role='provider' if kind=='provider_drain' else 'reservoir'
        prefix(host.anchors[role]['log'])
        details=drain(host,role,old[role],STATE,provider_identity=old['provider'] if role=='reservoir' else None,
                      reservoir_identity=old['reservoir'],wait_applied=produce and role=='provider')
        # Keep exact rows independently of later appended replacement events.
        ref=preserve(host.root/(kind+'-events.json'),details,produce)
        evidence.update(details=details,retained=ref)
    elif kind=='backup':
        for role in old:host.absent(role)
        r.require(host.receipts['final_snapshot']['passed'] is True,'final snapshot prerequisite missing')
        payload=backup_payload(host)
        ref=preserve(host.root/'fixture-backup.json',payload,produce)
        evidence.update(backup=ref,expected_state=STATE,all_files=True)
    elif kind=='private_restore':
        for role in old:host.absent(role)
        payload=r.check_reference(host.receipts['backup']['evidence']['backup'])
        r.require(payload==backup_payload(host),'stopped fixture bytes changed before private JSON restore')
        restored=copy.deepcopy(payload['files']['state.json']['value'])
        r.require(restored==STATE,'private JSON restore differs')
        ref=preserve(host.root/'fixture-private-restore.json',{'state':restored,'backup_sha256':r.digest(payload),
                     'json_only':True,'numeric_qualification':False},produce)
        evidence.update(restore=ref,expected_state=STATE,json_only=True)
    elif kind in ('startup','provider_ready'):
        role='reservoir' if kind=='startup' else 'provider';identity=host.startups[role]['identity']
        prefix(host.startups[role]['log_anchor'])
        details=ready(host,role,identity)
        if kind=='startup':host.absent('provider')
        else:ready(host,'reservoir',host.startups['reservoir']['identity'])
        evidence.update(details=details,retained=preserve(host.root/(kind+'-events.json'),details,produce))
    value={'schema':'reservoir_maintenance_witness_v1','kind':kind,'transaction_id':host.token,
           'passed':True,'bindings':copy.deepcopy(context['bindings']),'evidence':evidence}
    if not produce:
        if outer.exists():r.require(r.read(outer)==value,'canonical fixture witness differs')
    return value


def validate_saved(host,kind,receipt):
    """Validate immutable accepted evidence after live replacements can advance."""
    r.require(receipt.get('schema')=='reservoir_maintenance_witness_v1' and receipt.get('kind')==kind
              and receipt.get('passed') is True and receipt.get('transaction_id')==host.token
              and receipt.get('bindings')==context_for(host,kind)['bindings']
              and receipt.get('evidence',{}).get('scope')=='owned_fixture_only'
              and receipt['evidence'].get('production_qualified') is False,'retained witness envelope differs')
    evidence=receipt['evidence'];outer=host.root/(kind+'-witness.json')
    r.require(r.read(outer)==receipt,'accepted outer witness changed')
    if kind in ('provider_drain','final_snapshot','startup','provider_ready'):
        details=r.check_reference(evidence['retained'])
        r.require(details==evidence['details'],'retained event details changed')
        identity=details['identity'];role='provider' if kind in ('provider_drain','provider_ready') else 'reservoir'
        current=host.events(role,identity['pid'])
        saved=details['events']
        r.require(current[:len(saved)]==saved,'accepted worker event prefix changed')
        if kind in ('provider_drain','final_snapshot'):
            r.require(current==saved and host.io.process(identity['pid']) is None,'drained old worker history changed')
            prefix(host.anchors[role]['log'])
            if role=='provider':
                r.check_reference(details['request']);r.check_reference(details['ack'])
            # A later reservoir state may contain new work. Never roll it back.
        else:prefix(host.startups[role]['log_anchor'])
    elif kind=='backup':
        payload=r.check_reference(evidence['backup'])
        r.require(payload['expected_state']==STATE and payload['files']['state.json']['value']==STATE,'backup state differs')
    elif kind=='private_restore':
        value=r.check_reference(evidence['restore'])
        r.require(value['state']==STATE and value['json_only'] is True
                  and value['backup_sha256']==r.digest(r.check_reference(host.receipts['backup']['evidence']['backup'])),
                  'private fixture restore differs')
    else:raise ValueError('unsupported fixture witness')
    return receipt
