"""Separate, finite second-drain cleanup evidence for the two owned jobs."""
from __future__ import annotations
import copy

import owned_fixture_records as r
from owned_fixture_retained import history, adopt_operation, OPS
from owned_fixture_witnesses import drain, STATE


def enable_cleanup(host,context,authorizer):
    host.core['transaction_execution'].require_active(host.root,host.plan)
    from owned_fixture_cleanup import attach
    actual,ledger=attach(host,host.core)
    r.require(actual==context and callable(authorizer),'cleanup context/claim authorizer differs')
    expected=dict(host.plan['input_hashes']);expected['reservoir_source']=expected['reservoir_candidate']
    r.require(host.inputs()==expected,'cleanup accepted source/settings changed')
    host.cleanup=copy.deepcopy(context);host.cleanup_authorizer=authorizer
    hist=history(host);done=ledger.completed()
    for role in ('provider','reservoir'):
        path=host.control(role)/'hold.json'
        guard=r.read(path) if path.exists() else None
        r.require(guard in (None,context['guard']),'foreign cleanup hold')
        host.guards[role]=guard
    for (operation,role),pair in hist['pairs'].items():
        if operation in ('debug_intent','signal_intent'):
            ident=context['main_identities'][role]
            r.require(pair['intent']['identity']==ident,'cleanup command targeted different worker')
            (host.armed if operation=='debug_intent' else host.signalled).add((role,ident['pid'],ident['started_at']))
    for action,result in done.items():
        if action.startswith('held_verified:'):
            role=action.split(':')[1];r.identity(result['identity'])
    validate_roles(host,context,ledger,done)
    # No main operation counter, anchor, witness, or completion is rewritten.
    return context


def validate_roles(host,context,ledger,done):
    """Guard both jobs before any one-role cleanup producer or control call."""
    claim=ledger.rows[-1] if ledger.rows else {}
    operation,_,target=claim.get('action','').partition(':')
    reconciling=claim.get('type')=='reconcile_claim'
    for role in ('provider','reservoir'):
        old=context['main_identities'][role];view=host.inspect_role(role)
        absent=not view['job_loaded'] and view['process'] is None and not view['listener_pids']
        held=view.get('held')
        exact_held=bool(held and held['guard']==context['guard'] and not view['listener_pids']
                        and host.io.process(old['pid']) is None)
        accepted=done.get('held_verified:'+role)
        if accepted:exact_held=exact_held and view['process']==accepted['identity']
        removed='remove_held:'+role in done
        if removed:r.require(absent,'cleanup parked role changed: '+role);continue
        if accepted:
            r.require(exact_held or reconciling and target==role and operation=='remove_held' and absent,
                      'cleanup accepted holder changed: '+role)
            continue
        signal_attempt='signal_stop:'+role in done or any(row.get('action')=='signal_stop:'+role
                    and row.get('type')=='claim' for row in ledger.rows)
        effect=done.get('arm_next_spawn:'+role,{}).get('effect')=='owned_holder_observed'
        pending_effect=reconciling and target==role and operation in ('arm_next_spawn','signal_stop','held_verified')
        publication_gap=(target==role and operation=='held_verified' and 'signal_stop:'+role in done
                         and view['job_loaded'] and view['process'] is None and held is None
                         and not view['listener_pids'] and host.io.process(old['pid']) is None)
        exact_old=(view['process']==old and view['job_loaded'] and not held
                   and (view['listener_pids']==[old['pid']] or signal_attempt and not view['listener_pids']))
        r.require(exact_old or (signal_attempt or effect or pending_effect) and exact_held or publication_gap,
                  'cleanup original or expected drain role changed: '+role)


def require_context(host,context):
    host.core['transaction_execution'].require_active(host.root,host.plan)
    r.require(host.cleanup==context and r.read(host.root/'cleanup/context.json')==context,
              'cleanup is not attached to exact retained context')
    r.check_reference(context['main_backup']['evidence']['backup'])
    expected=dict(host.plan['input_hashes']);expected['reservoir_source']=expected['reservoir_candidate']
    r.require(host.inputs()==expected,'cleanup accepted source/settings changed')


def held(host,role,context):
    require_context(host,context)
    old=context['main_identities'][role]
    def observed():
        view=host.inspect_role(role)
        if view['held'] and not view['listener_pids'] and host.io.process(old['pid']) is None:
            r.require(view['held']['guard']==context['guard'],'cleanup holder owner differs')
            return {'identity':view['process']}
        return None
    return host.wait(observed,'cleanup exact holder')


def cleanup_witness(host,role,context,*,produce=False):
    require_context(host,context)
    if produce:host._allow('cleanup_witness',role)
    expected=context['expected_ticks'];old=context['main_identities'][role]
    view=host.inspect_role(role)
    r.require(view['held'] and view['held']['guard']==context['guard'] and not view['listener_pids'],
              'cleanup witness requires exact owned holder')
    details=drain(host,role,old,expected,provider_identity=context['main_identities']['provider'] if role=='reservoir' else None,
                  reservoir_identity=context['main_identities']['reservoir'],wait_applied=produce and role=='provider')
    original=context['main_backup'];r.require(original['evidence']['expected_state']==STATE,'main backup baseline differs')
    backup=r.check_reference(original['evidence']['backup'])
    r.require(backup['expected_state']==STATE,'main backup changed')
    value={'schema':'owned_fixture_cleanup_witness_v1','role':role,'passed':True,
           'context_sha256':r.digest(context),'details':details,'main_backup_unchanged':True,
           'scope':'owned_fixture_only','production_qualified':False}
    path=host.root/'cleanup'/(role+'-drain-witness.json')
    if produce:
        ref=r.write(path,value)
        if host.boundary_hook:host.boundary_hook('cleanup:witness:'+role,ref)
    else:r.require(r.read(path)==value,'saved cleanup witness differs')
    return value


def final(host,context,*,require_guards_absent=False):
    require_context(host,context)
    identities={}
    for role in ('provider','reservoir'):
        host.absent(role)
        identities[host.plan['roles'][role]['identity']['pid']]=host.plan['roles'][role]['identity']
        identities[context['main_identities'][role]['pid']]=context['main_identities'][role]
        if require_guards_absent:r.require(not (host.control(role)/'hold.json').exists(),'cleanup guard remains')
    for directory in (host.root,host.root/'cleanup'):
        rows=history(host,directory=directory)['events']
        for row in rows:
            if 'identity' in row:
                ident=r.identity(row['identity']);identities[ident['pid']]=ident
    for pid in identities:r.require(host.io.process(pid) is None,'owned worker/holder or reused PID survives cleanup')
    r.require(r.read(host.root/'state.json')==context['expected_ticks'],'final 22/7 state differs')
    applied=[row for row in host.events('reservoir',complete=True) if row['phase']=='checkin_applied']
    requests=[row['request_id'] for row in applied]
    r.require(len(requests)==len(set(requests))==2 and len(list(host.root.glob('request.*.json')))==2
              and len(list(host.root.glob('ack.*.json')))==2,'cleanup check-ins missing or duplicated')
    r.require([row['state']['fixture_astrid']['ticks'] for row in applied]==[11,22],'cleanup check-in progression differs')
    for role in ('provider','reservoir'):
        witness=r.read(host.root/'cleanup'/(role+'-drain-witness.json'))
        r.require(witness.get('passed') is True and witness.get('context_sha256')==r.digest(context),'cleanup drain witness missing')
    backup=r.check_reference(context['main_backup']['evidence']['backup'])
    r.require(backup['expected_state']==STATE and backup['files']['state.json']['value']==STATE,'original 11/7 backup changed')
    return {'passed':True,'both_jobs_absent':True,'all_owned_pids_absent':True,'all_ports_unowned':True,
            'final_state':context['expected_ticks'],'main_backup_unchanged':True,'unique_applied_checkins':2,
            'force_used':False,'production_qualified':False,'scope':'owned_fixture_only'}


def reconcile_cleanup(host,action_id,context,original_claim):
    require_context(host,context)
    ledger=r.Ledger(host.root,'cleanup',{'context_sha256':r.digest(context)})
    claim=ledger.rows[-1] if ledger.rows else {}
    r.require(claim.get('type')=='reconcile_claim' and claim.get('action')==action_id
              and claim.get('executor_identity')==host.executor_identity()
              and original_claim in ledger.rows and original_claim['type']=='claim'
              and original_claim['action']==action_id,'cleanup lacks exact observed-adoption claim')
    operation,_,role=action_id.partition(':');role=role or None
    hist=history(host);done=ledger.completed();key=next((k for k,v in OPS.items() if v==operation),None)
    pair=hist['pairs'].get((key,role)) if key else None
    if operation=='cleanup_witness':
        result=cleanup_witness(host,role,context,produce=False);evidence={'unchanged_saved_witness':True}
    elif operation in ('final_state','complete'):
        result=final(host,context,require_guards_absent=operation=='complete');evidence={'current_final_observation':True}
    else:
        holders={name:done['held_verified:'+name]['identity'] for name in ('provider','reservoir')
                 if 'held_verified:'+name in done}
        result,evidence=adopt_operation(host,operation,role,pair=pair,old=context['main_identities'][role],
            guard=context['guard'],holders=holders,
            allow_held_effect=done.get('arm_next_spawn:'+role,{}).get('effect')=='owned_holder_observed')
    return {'passed':True,'action_id':action_id,'original_claim_sha256':r.digest(original_claim),
            'result':result,'evidence':evidence,'producer_replayed':False,'production_qualified':False}
