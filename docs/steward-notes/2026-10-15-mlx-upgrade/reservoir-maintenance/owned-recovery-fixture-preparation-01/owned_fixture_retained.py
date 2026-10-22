"""Read actual owned effects and retain truthful one-step recovery provenance.

No calls to production inspection, no synthetic historical completions, and no
control calls from reconciliation. Importing this module performs no work.
"""
from __future__ import annotations
import copy
import datetime
import os
from pathlib import Path
import time

import owned_fixture_records as r
from owned_fixture_witnesses import (KINDS, STATE, context_for, make_witness,
                                     prefix, ready, validate_saved)

PAIRS={'hold_intent':'hold_created','debug_intent':'debug_result','signal_intent':'signal_sent',
       'held_bootout_intent':'held_job_absent','install_intent':'install_completed',
       'bootstrap_intent':'bootstrap_ready','hold_release_intent':'hold_released'}
OPS={'hold_intent':'create_hold','debug_intent':'arm_next_spawn','signal_intent':'signal_stop',
     'held_bootout_intent':'remove_held','install_intent':'install_repair',
     'bootstrap_intent':'bootstrap_original','hold_release_intent':'release_hold'}


def operation_action(operation,role):
    first=operation if operation in PAIRS else next((key for key,value in PAIRS.items() if value==operation),None)
    if operation=='bootstrap_command_returned':first='bootstrap_intent'
    r.require(first is not None,'unknown fixture Host operation')
    return OPS[first]+(':'+role if role else '')


def action_authority(host,operation,role):
    action=operation_action(operation,role)
    if host.cleanup:
        ledger=r.Ledger(host.root,'cleanup',{'context_sha256':r.digest(host.cleanup)})
        row=ledger.rows[-1] if ledger.rows else {}
        r.require(row.get('type')=='claim' and row.get('action')==action,'cleanup Host operation lacks claim')
        return {'stream':'cleanup','record_sha256':r.digest(row),'action_id':action}
    journal=host.m.Journal(host.root/'controller')
    ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
    if ledger['records']:
        row=ledger['records'][-1]
        r.require(row.get('type')=='action_claim' and row.get('action_id')==action,
                  'Host operation lacks exact new recovery claim')
        return {'stream':'recovery','record_sha256':r.digest(row),'action_id':action}
    row=journal.events[-1] if journal.events else {}
    r.require(row.get('type')=='mutation_intent' and row.get('operation')==action.split(':')[0]
              and row.get('role')==role,'Host operation lacks original mutation intent')
    return {'stream':'controller','record_sha256':r.digest(row),'action_id':action}


def history(host,*,directory=None,validate=True):
    directory=Path(directory) if directory else host.root/'cleanup' if host.cleanup else host.root
    paths=sorted(directory.glob('host-operation-*.json'));r.require(len(paths)<=500,'Host history exceeds bound')
    rows=[];pairs={};open_pair=None
    authorities={}
    if validate and directory==host.root:
        journal=host.m.Journal(host.root/'controller')
        ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
        for stream,records in (('controller',journal.events),('recovery',ledger['records'])):
            authorities.update({(stream,r.digest(row)):row for row in records})
    elif validate and directory==host.root/'cleanup':
        ledger=r.Ledger(host.root,'cleanup',{'context_sha256':r.digest(host.cleanup)})
        authorities={('cleanup',r.digest(row)):row for row in ledger.rows}
    for index,path in enumerate(paths,1):
        r.require(path.name==f'host-operation-{index:05d}.json','Host operation history gap')
        row=r.read(path);rows.append(row);operation=row.get('operation');role=row.get('role')
        r.require(role in ('provider','reservoir',None) and type(row.get('wall_time_ns')) is int
                  and row['wall_time_ns']>0,'Host role/time invalid')
        action=operation_action(operation,role)
        if validate:
            authority=row.get('authority',{});source=authorities.get((authority.get('stream'),authority.get('record_sha256')))
            r.require(source is not None and authority.get('action_id')==action,'Host event has no retained action authority')
            if authority['stream']=='controller':
                r.require(source.get('type')=='mutation_intent' and source.get('operation')==action.split(':')[0]
                          and source.get('role')==role,'Host controller authority differs')
            else:r.require(source.get('type') in ('action_claim','claim')
                           and source.get('action_id',source.get('action'))==action,'Host claim authority differs')
        key=(operation,role)
        if operation in PAIRS:
            # A prior incomplete result may be adopted; a later distinct action
            # retains that historical absence, never fills it with a fake row.
            r.require(key not in pairs,'Host producer intent repeated')
            pair={'intent':row,'result':None,'command_return':None};pairs[key]=pair;open_pair=key
            if validate:
                cleanup_scope=directory==host.root/'cleanup'
                guard=host.cleanup['guard'] if cleanup_scope else {
                    'schema':'model_environment_hold_v1','transaction_id':host.token,'actor':host.plan['actor']}
                old=(host.cleanup['main_identities'] if cleanup_scope else {
                    name:host.plan['roles'][name]['identity'] for name in ('provider','reservoir')})
                r.require(role in ('provider','reservoir') if operation!='install_intent' else role is None,
                          'Host operation role differs')
                if operation in ('hold_intent','debug_intent','hold_release_intent'):
                    r.require(row.get('guard')==guard,'Host intent guard differs')
                if operation in ('debug_intent','signal_intent'):
                    r.require(row.get('identity')==old[role],'Host intent original identity differs')
                if operation=='debug_intent':
                    r.require(row.get('arguments')==['--program',str(host.executable),'--',*host.hold_argv(role,guard)[1:]],
                              'Host debug override differs')
                if operation=='signal_intent':r.require(row.get('signal')=='SIGTERM','Host signal changed')
                if operation=='bootstrap_intent':
                    r.require(row.get('expected_argv')==host.argv(role)
                              and row.get('original_rollback_plist_sha256')==host.plan['input_hashes'][role+'_settings']
                              and row.get('applied_plist_sha256')==host.plan['input_hashes'][role+'_settings'],
                              'Host bootstrap settings differ')
                    prefix(row['log_anchor'])
                if operation=='install_intent':
                    r.require(row.get('source_sha256')==host.plan['input_hashes']['reservoir_candidate']
                              and row.get('target')==str(host.root/'reservoir-source.marker'),'private install intent differs')
        elif operation=='bootstrap_command_returned':
            key=('bootstrap_intent',role);pair=pairs.get(key)
            r.require(pair is not None and pair['command_return'] is None and pair['result'] is None,'bootstrap return out of order')
            pair['command_return']=row
        else:
            key=(next(name for name,value in PAIRS.items() if value==operation),role);pair=pairs.get(key)
            r.require(pair is not None and pair['result'] is None and open_pair==key,'Host completion out of order')
            if validate:r.require(pair['intent']['authority']==row['authority'],'Host completion belongs to different claim')
            pair['result']=row;open_pair=None
    return {'events':rows,'pairs':pairs,'refs':{str(path):r.reference(path) for path in paths}}


def latest_pair(host,operation,role):return history(host)['pairs'].get((operation,role))


def born_after(identity,boundary):
    r.identity(identity)
    try:born=time.mktime(datetime.datetime.strptime(identity['started_at'],'%a %b %d %H:%M:%S %Y').timetuple())
    except (TypeError,ValueError) as error:raise ValueError('exact OS birth time unavailable') from error
    r.require(type(boundary) is int and boundary>0 and born+1>=boundary/1e9 and born<=time.time()+1,
              'worker predates retained bootstrap intent')
    return born


def prior_ready_during_bootstrap(host,role,process,record):
    if host.plan is None:return False
    old=host.plan['roles'][role]['identity']
    pair=history(host)['pairs'].get(('bootstrap_intent',role))
    if pair is None or pair['result'] is not None:return False
    r.require(record.get('pid')==old['pid'] and record.get('port')==host.spec['anchors'][role]['sentinel_port']
              and host.io.process(old['pid']) is None,'ready marker is not proven prior owned worker')
    born_after(process,pair['intent']['wall_time_ns'])
    return True


def setup_effect(host,role,original):
    host.validate_setup_owner(host.owner,require_executor=False)
    ledger=r.Ledger(host.root,'setup',{'owner_sha256':r.digest(host.owner),'release_sha256':host.owner['release_ref']['sha256']})
    claim=ledger.rows[-1] if ledger.rows else {}
    r.require(claim.get('type')=='reconcile_claim' and claim.get('action')=='bootstrap:'+role
              and claim.get('executor_identity')==host.executor_identity()
              and original in ledger.rows and original['type']=='claim'
              and original['action']==claim['action']
              and all(claim.get(key)==original.get(key) for key in ('target','plist_ref','expected_argv')),
              'setup adoption lacks current exact reconciliation claim')
    r.require(claim['plist_ref']==r.reference(host.root/(role+'.plist')),'setup adoption plist changed')
    pair=history(host,directory=host.root/'setup-host',validate=False)['pairs'].get(('bootstrap_intent',role))
    r.require(pair is not None and pair['intent'].get('claim_sha256')==r.digest(original),
              'setup attempted command intent missing')
    view=host.setup_observe(role);r.require(view.get('ready') is True,'setup effect is not ready; never bootstrap again')
    born=born_after(view['process'],pair['intent']['wall_time_ns'])
    result={'identity':view['process'],'sentinel_port':host.ports[role],'ready':True,'scope':'owned_fixture_only'}
    return result,{'observed':view,'birth_wall_seconds':born,'intent':pair['intent'],
                   'historical_command_return_inferred':False,'producer_replayed':False}


def snapshot(host):
    host.check_owned()
    return {'host':host.binding,'protected':host.plan['protected'],
            'stewardship':{'paused':True,'pause_generation':host.plan['pause_generation'],
                          'pause_actor':host.plan['actor'],'lease':None},
            'ownership_scope':'private fixture marker, not production steward authority'}


def inspect(host,journal):
    r.require(host.cleanup is None,'main inspection cannot run through cleanup phase')
    r.require(journal.root==host.root/'controller' and journal.plan==host.plan,'wrong main journal')
    before=snapshot(host);ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
    hist=history(host);signal_times={};roles={};artifacts=dict(hist['refs'])
    # Reserve every durable attempt before observing a draining worker.
    for (operation,role),pair in hist['pairs'].items():
        if operation in ('debug_intent','signal_intent'):
            ident=pair['intent']['identity'];key=(role,ident['pid'],ident['started_at'])
            (host.armed if operation=='debug_intent' else host.signalled).add(key)
            if operation=='signal_intent':signal_times[role]=pair['intent']['wall_time_ns']
    for role in ('provider','reservoir'):
        prefix(host.anchors[role]['log']);view=host.inspect_role(role);old=host.plan['roles'][role]['identity']
        old_seen=host.io.process(old['pid']);r.require(old_seen in (None,old),'original PID reused or identity changed')
        guard_path=host.control(role)/'hold.json';guard=r.read(guard_path) if guard_path.exists() else None
        if guard is not None:r.require(guard==host.guard(),'foreign main guard')
        if view['held']:classification='owned_holder_observed'
        elif view['process']==old:classification='original_identity_observed'
        elif view['process'] is None and not view['job_loaded'] and not view['listener_pids']:classification='absent_observed'
        elif view['process'] is not None:
            pair=hist['pairs'].get(('bootstrap_intent',role));r.require(pair is not None,'replacement lacks retained birth intent')
            born_after(view['process'],pair['intent']['wall_time_ns'])
            r.require(view['process']['argv']==old['argv'] and old_seen is None,'replacement arguments/original exit differ')
            classification='recorded_replacement_observed' if pair['result'] else 'unrecorded_replacement_candidate'
        else:classification='publication_gap'
        roles[role]={'guard':guard,'observed':view,'old_pid_observation':old_seen,'classification':classification}
    accepted={}
    for row in ledger['records']:
        if row['type']=='reconciliation_result':accepted[row['action_id']]=row
    unresolved=[];historical=[]
    for (operation,role),pair in hist['pairs'].items():
        action=OPS[operation]+(':'+role if role else '')
        missing=pair['result'] is None or operation=='debug_intent' and pair['result'].get('returncode')!=0
        if missing:(historical if action in accepted else unresolved).append({'action_id':action,'intent':pair['intent']})
    source=r.reference(host.root/'reservoir-source.marker')['sha256'];inputs=host.plan['input_hashes']
    r.require(source in (inputs['reservoir_source'],inputs['reservoir_candidate']),'unknown private installed marker')
    for role in ('provider','reservoir'):
        r.require(r.reference(host.root/(role+'.plist'))['sha256']==inputs[role+'_settings'],'private plist changed')
    for path in (host.root/'setup-complete.json',host.root/'preparation.json',host.root/'plan.json'):
        artifacts[str(path)]=r.reference(path)
    for kind in KINDS:
        for path in (host.root/(kind+'-witness.json'),host.root/(kind+'-events.json'),host.root/(kind+'-witness-intent.json')):
            if path.exists():artifacts[str(path)]=r.reference(path)
    for name in ('fixture-backup.json','fixture-private-restore.json'):
        path=host.root/name
        if path.exists():artifacts[str(path)]=r.reference(path)
    report={'schema':'owned_fixture_recovery_inspection_v1','evidence_valid':True,'errors':[],
            'transaction_id':host.token,'preparation_sha256':r.digest(host.spec),
            'observed_before':before,'observed_after':snapshot(host),'capture_anchors':copy.deepcopy(host.anchors),
            'controller':{'events':copy.deepcopy(journal.events),'last_sha256':journal.previous},
            'host_operations':{'events':hist['events']},'signal_wall_time_ns':signal_times,'roles':roles,
            'unresolved':unresolved,'historical_unresolved':historical,'recovery_ledger':ledger,'artifacts':artifacts,
            'installed_variants':{'provider':{'source':'original','plist':'original'},'reservoir':{
                'source':'candidate' if source==inputs['reservoir_candidate'] else 'original','plist':'original'}},
            'recovery_output_presence':{kind:any((host.root/name).exists() for name in
                (kind+'-witness.json',kind+'-events.json',kind+'-witness-intent.json',*(('fixture-backup.json',) if kind=='backup' else
                 ('fixture-private-restore.json',) if kind=='private_restore' else ()))) for kind in KINDS},
            'production_qualified':False}
    r.require(report['observed_before']==report['observed_after'],'fixture host changed during inspection')
    for role in roles:r.require(host.inspect_role(role)==roles[role]['observed'],'role changed during inspection')
    checkpoint=host.core['recovery_phases'].checkpoint_from_inspection(host.spec,report,ledger)
    # Pure core checks hash chains, sessions and exact appended spans. Also bind
    # every raw file digest (the pure core deliberately cannot read files).
    for row in ledger['records']:
        for ref in row.get('host_operations',{}).get('appended',[]):
            r.require(r.reference(ref['path'])['sha256']==ref['sha256'],'recovery Host file digest changed')
    for role,ident in checkpoint['replacements'].items():
        r.require(roles[role]['observed']['process']==ident,'accepted replacement changed')
    return report


def revalidate(host,checkpoint,inspection):
    host.core['transaction_execution'].require_active(host.root,host.plan)
    r.require(host.expected_recovery_action==checkpoint['action_id'],'planned fixture action differs before claim')
    journal=host.m.Journal(host.root/'controller');current=inspect(host,journal)
    r.require(current==inspection,'retained fixture inspection changed')
    ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
    r.require(host.core['recovery_phases'].checkpoint_from_inspection(host.spec,current,ledger)==checkpoint,
              'fixture checkpoint is not authoritative reconstruction')
    host.guards=copy.deepcopy(checkpoint['guards']);host.receipts=copy.deepcopy(checkpoint['receipts'])
    host.startups=copy.deepcopy(checkpoint['startups']);host.repaired=checkpoint['repaired']
    for kind in KINDS:
        if kind in host.receipts:validate_saved(host,kind,host.receipts[kind])
    for role,ident in checkpoint['replacements'].items():ready(host,role,ident)
    host._validated={'checkpoint':copy.deepcopy(checkpoint),'inspection':copy.deepcopy(inspection)}
    return checkpoint


def bootstrap_effect(host,role,pair):
    r.require(pair is not None,'no retained bootstrap intent')
    old=host.plan['roles'][role]['identity'];view=host.setup_observe(role)
    r.require(view.get('ready') is True and host.io.process(old['pid']) is None
              and view['process']['argv']==old['argv'] and view['process']!=old,'bootstrap effect is not exact ready replacement')
    born_after(view['process'],pair['intent']['wall_time_ns']);prefix(pair['intent']['log_anchor'])
    details=ready(host,role,view['process'])
    host.startups[role]={'identity':view['process'],'log_anchor':pair['intent']['log_anchor']}
    if role=='reservoir':host.absent('provider')
    else:r.require(host.receipts.get('startup',{}).get('passed') is True,'prior reservoir startup missing')
    return {'identity':view['process']},{'observed':view,'ready':details,'historical_command_return_inferred':False}


def adopt_operation(host,operation,role,*,pair,old,guard,holders,allow_held_effect=False):
    """Shared narrow effect proof for main and separately owned cleanup."""
    if operation in ('arm_next_spawn','signal_stop','held_verified'):
        r.require(operation=='held_verified' or pair is not None
                  or operation=='signal_stop' and allow_held_effect,'no retained command intent')
        view=host.inspect_role(role);record=pair['result'] if pair else None
        if operation=='arm_next_spawn' and record and record.get('returncode')==0 and view['process']==old:
            host.env.assert_hold(host.control(role),guard)
            r.require(view['listener_pids']==[old['pid']],'armed original lost listener')
            return {'armed':True},{'recorded_result':record,'observed':view}
        if operation=='signal_stop' and record and view['process']==old:
            r.require(record.get('identity')==old,'signal receipt identity differs')
            return {'signal':'SIGTERM','identity':old,'force_used':False},{'recorded_result':record,'observed':view}
        r.require(view['held'] and view['held']['guard']==guard and not view['listener_pids']
                  and host.io.process(old['pid']) is None,'unknown command outcome; exact owned holder not observed')
        value={'identity':view['process']} if operation=='held_verified' else {
            'effect':'owned_holder_observed','identity':view['process'],'original_identity':old,'guard':guard}
        return value,{'observed':view,'historical_signal_or_debug_return_inferred':False}
    if operation=='create_hold':
        r.require(pair is not None and pair['intent']['guard']==guard,'hold intent missing/different')
        host.env.assert_hold(host.control(role),guard)
        return {'guard':guard},{'guard_observed':True}
    if operation in ('remove_held','job_absent'):
        host.absent(role)
        if operation=='remove_held':
            r.require(pair is not None and pair['intent']['identity']==holders[role],'removal target differs')
            r.require(host.io.process(holders[role]['pid']) is None,'removed holder survives')
            return {'removed_identity':holders[role]},{'job_holder_listener_absent':True}
        return {'absent':True},{'job_listener_absent':True}
    if operation=='release_hold':
        r.require(pair is not None and pair['intent']['guard']==guard
                  and not (host.control(role)/'hold.json').exists(),'release effect unproved')
        return {'released':True},{'guard_absent':True}
    raise ValueError('unsupported common adoption')


def reconcile(host,action_id,checkpoint,inspection):
    session=host.core['transaction_execution'].require_active(host.root,host.plan)
    r.require(host._validated=={'checkpoint':checkpoint,'inspection':inspection},'reconciliation was not revalidated')
    ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan);claim=ledger['records'][-1]
    r.require(claim['type']=='reconciliation_claim' and claim['action_id']==action_id
              and claim['checkpoint_sha256']==r.digest(checkpoint) and claim['inspection_sha256']==r.digest(inspection)
              and claim['executor_session']==session.begin,'wrong reconciliation claim')
    hist=history(host);r.require(hist['events']==inspection['host_operations']['events'],'Host history changed after claim')
    operation,_,role=action_id.partition(':');role=role or None
    r.require(claim['context']==context_for(host,role if operation=='witness' else None),'reconciliation context differs')
    key=next((key for key,value in OPS.items() if value==operation),None)
    pair=hist['pairs'].get((key,role)) if key else None
    if operation=='witness':
        result=make_witness(host,role,context_for(host,role),produce=False)
        outer=host.root/(role+'-witness.json')
        if not outer.exists():r.write(outer,result)
        evidence={'retained_witness':r.reference(outer),'producer_replayed':False}
    elif operation=='preflight':
        result=host.m.preflight(host.plan,host);evidence={'current_read_only_observation':True}
    elif operation=='completed':
        for name in ('provider','reservoir'):
            ready(host,name,host.startups[name]['identity'])
            r.require(not (host.control(name)/'hold.json').exists(),'guard still present')
        result={'schema':'reservoir_maintenance_result_v1','status':'verified','transaction_id':host.token,
                'plan_sha256':r.digest(host.plan),**{name:host.startups[name]['identity'] for name in ('provider','reservoir')},
                'witness_hashes':{kind:r.digest(value) for kind,value in host.receipts.items()},
                'force_used':False,'runtime_state_rollback':False,'execution_scope':'owned_fixture',
                'production_qualified':False,'no_missed_input_claimed':False}
        evidence={'current_completion_observed':True}
    elif operation=='install_repair':
        for name in ('provider','reservoir'):host.absent(name)
        r.require(pair is not None and r.reference(host.root/'reservoir-source.marker')['sha256']==host.plan['input_hashes']['reservoir_candidate']
                  and host.receipts.get('private_restore',{}).get('passed') is True,'private repair effect unproved')
        result={'source_sha256':host.plan['input_hashes']['reservoir_candidate']};evidence={'exact_private_marker_observed':True}
    elif operation=='bootstrap_original':result,evidence=bootstrap_effect(host,role,pair)
    else:
        if operation=='release_hold':
            for name in ('provider','reservoir'):ready(host,name,host.startups[name]['identity'])
        result,evidence=adopt_operation(host,operation,role,pair=pair,
            old=host.plan['roles'][role]['identity'],guard=checkpoint['guard'],holders=checkpoint['holders'],
            allow_held_effect=role in checkpoint['held_effects'])
    r.require(snapshot(host)==inspection['observed_after'],'fixture ownership changed during reconciliation')
    return {'schema':'reservoir_recovery_reconciliation_v1','passed':True,'action_id':action_id,
            'transaction_id':host.token,'plan_sha256':r.digest(host.plan),'checkpoint_sha256':r.digest(checkpoint),
            'inspection_sha256':r.digest(inspection),'result':result,'evidence':evidence}
