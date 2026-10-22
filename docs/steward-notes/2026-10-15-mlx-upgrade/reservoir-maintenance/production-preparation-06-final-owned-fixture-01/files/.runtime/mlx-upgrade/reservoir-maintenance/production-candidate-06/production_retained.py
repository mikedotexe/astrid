"""Retained Host attachment and exact-effect adoption; no producer replay.

Only explicit methods observe state. All action-capable paths retain the false
production gate and shared executor lock. No numeric libraries are imported.
"""
from __future__ import annotations
import copy
import datetime
import os
from pathlib import Path
import re
import time

import production_preflight as p
import reservoir_maintenance_state as state

m = p.m


def attach(cls, preparation_path, journal_path, *, io=None):
    from production_recovery_inspect import Evidence, validate_retained, anchors
    evidence = Evidence(); path = Path(preparation_path)
    spec = evidence.read(path, record=True); root = Path(spec['transaction_root'])
    plan, _, _ = validate_retained(spec, root, evidence)
    m.require(Path(journal_path) == root / 'controller', 'wrong retained controller path')
    # This validates preserved originals/current known variants, not fresh-live
    # source alignment or a second capture. No service observation occurs here.
    saved_anchors = anchors(root, plan, evidence); evidence.finish()
    host = cls.__new__(cls)
    host.spec, host.plan, host.root = spec, plan, root
    host.preparation_path, host.journal_path = path, Path(journal_path)
    host.io = io or p.ReadOnlySystem(); host.env = p.load_hold()
    from production_restore_runner import FiniteRestoreRunner
    host.restore_runner = FiniteRestoreRunner(host)
    host.guards, host.armed, host.signalled = {}, set(), set()
    host.anchors, host.receipts, host.startups = saved_anchors, {}, {}
    host.repaired = host.provider_pinned = False
    host.signal_times = {}; host.counter = 0
    from production_host import stamp
    host.bound_stats = {name: stamp(name) for name in spec['immutable_files']}
    host._recovery_validated = None
    return host


def require_session(host):
    import production_host
    m.require(production_host.PRODUCTION_EXECUTION_ENABLED is True, 'production execution disabled: retained recovery')
    from transaction_execution import require_active
    return require_active(host.root, host.plan)


def revalidate(host, checkpoint, inspection):
    require_session(host)
    from recovery_ledger import read_ledger
    from production_recovery import checkpoint_from_inspection
    ledger = read_ledger(host.root, host.plan)
    m.require(inspection.get('evidence_valid') is True and not inspection.get('errors')
              and inspection.get('recovery_ledger') == ledger, 'retained inspection/ledger incomplete or stale')
    expected = checkpoint_from_inspection(host.spec, inspection, ledger)
    m.require(expected == checkpoint, 'retained checkpoint differs from authoritative reconstruction')
    m.require(checkpoint['transaction_id'] == host.plan['transaction_id']
              and checkpoint['plan_sha256'] == m.digest(host.plan)
              and checkpoint['preparation_sha256'] == p.preparation_digest(host.spec), 'checkpoint belongs elsewhere')
    # Rehash all inspected retained artifacts, including original anchors. Live
    # evolving logs/state are deliberately not substituted for captured bytes.
    from production_recovery_inspect import Evidence
    evidence = Evidence()
    for path, ref in inspection['artifacts'].items():
        m.require(evidence.read(path, allow_alias=bool(ref.get('immutable_alias'))) == ref['sha256'],
                  'retained inspected artifact changed')
    evidence.finish()
    m.require(host.recovery_inspection(host.journal_path) == inspection, 'retained inspection changed before hydration')
    host.counter = checkpoint['host_counter']
    host.guards = copy.deepcopy(checkpoint['guards']); host.anchors = copy.deepcopy(checkpoint['anchors'])
    host.receipts = copy.deepcopy(checkpoint['receipts']); host.signal_times = dict(checkpoint['signal_times'])
    host.startups = copy.deepcopy(checkpoint['startups'])
    host.armed = set(checkpoint['armed_attempts'])
    for role, ident in checkpoint['armed_identities'].items():host.armed.add((role, ident['pid'], ident['started_at']))
    host.signalled = {(role, ident['pid'], ident['started_at']) for role, ident in checkpoint['signalled_identities'].items()}
    for role in checkpoint.get('signal_attempts', []):
        ident=host.plan['roles'][role]['identity'];host.signalled.add((role,ident['pid'],ident['started_at']))
    host.repaired, host.provider_pinned = checkpoint['repaired'], checkpoint['provider_pinned']
    for kind in ('provider_drain','final_snapshot','backup','private_restore','startup','provider_ready'):
        if kind in host.receipts:
            m.require(validate_existing_witness(host,kind,context_for(host,kind))==host.receipts[kind],
                      'accepted retained witness differs from revalidated semantics')
    for role,ident in checkpoint['replacements'].items():ready_identity(host,role,ident)

    # Re-run immutable proof admission; the Host's ordinary _allow will enforce
    # phase ownership before any new operation. Exact-effect adoption separately
    # handles an applied disk effect that lacks its old completion record.
    host.production_admission(host.plan)
    host._recovery_validated = {'checkpoint': copy.deepcopy(checkpoint), 'inspection': copy.deepcopy(inspection),
                                'ledger_head': ledger['head_sha256']}
    return checkpoint


def context_for(host, kind=None):
    bindings = {'plan_sha256': m.digest(host.plan), **{role + '_old_identity_sha256':
                 m.digest(host.plan['roles'][role]['identity']) for role in m.ROLES}}
    if kind == 'backup':bindings['final_snapshot_sha256'] = m.digest(host.receipts['final_snapshot'])
    if kind == 'private_restore':bindings.update(backup_sha256=m.digest(host.receipts['backup']),
        repair_source_sha256=host.plan['input_hashes']['reservoir_candidate'],
        interpreter_sha256=host.plan['input_hashes']['interpreter'],
        configuration_sha256=host.plan['input_hashes']['reservoir_settings'])
    if kind == 'startup':bindings.update(private_restore_sha256=m.digest(host.receipts['private_restore']),
        reservoir_new_identity_sha256=m.digest(host.startups['reservoir']['identity']))
    if kind == 'provider_ready':
        bindings.update(startup_sha256=m.digest(host.receipts['startup']),
                        provider_new_identity_sha256=m.digest(host.startups['provider']['identity']))
    return {'transaction_id':host.plan['transaction_id'], 'bindings':bindings, 'execution_scope':host.scope,
            'production_qualified':False, 'plan':copy.deepcopy(host.plan), 'receipts':copy.deepcopy(host.receipts)}


def validate_span(anchor, span, retained):
    """Check saved span against original prefix and live append-only log bytes."""
    fd, info = p.open_log(anchor['path'])
    try:
        start, end = anchor['start_offset'], span['end_offset']
        m.require(span['start_offset'] == start and (info.st_dev, info.st_ino) ==
                  (anchor['device'], anchor['inode']) == (span['device'], span['inode'])
                  and start <= end <= info.st_size, 'retained span identity/bounds differ')
        prefix, _ = p.hash_prefix(fd, start)
        m.require(prefix == anchor['prefix_sha256'], 'original captured prefix changed')
        digest, last = p.hash_prefix(fd, end - start)
        data, ref = state.artifact(retained)
        m.require(ref['bytes'] == end-start and ref['sha256'] == digest == span['sha256']
                  and (not data or last == b'\n'), 'retained span does not match exact log interval')
        p.check_log(fd, anchor['path'], info, end)
    finally:os.close(fd)


def validate_existing_witness(host, kind, context, *, adoption=False):
    """Re-evaluate saved evidence; never recapture a producer's output."""
    outer = host.root / (kind + '-witness.json')
    if kind == 'private_restore':
        import restore_verifier as v
        path = host.restore_runner.adopt_existing(context, host.receipts['backup'], host.spec) if adoption else \
               Path(host.receipts['private_restore']['evidence']['work_root']) / 'restore-witness.json'
        receipt = v.validate_restore_witness(path, context, host.receipts['backup'])
    else:
        source = outer if outer.exists() else host.root / 'stopped-backup' / 'backup-receipt.json' if kind=='backup' else outer
        receipt = m.read_record(source)
    m.require(receipt.get('schema') == 'reservoir_maintenance_witness_v1' and receipt.get('kind') == kind
              and receipt.get('passed') is True and receipt.get('transaction_id') == context['transaction_id']
              and all(receipt.get('bindings',{}).get(k)==v for k,v in context['bindings'].items()),
              'retained witness binding incomplete')
    data = receipt['evidence']
    from production_recovery_inspect import Evidence, references
    evidence = Evidence(); references(receipt, evidence)
    if kind in ('provider_drain','final_snapshot'):
        role = 'provider' if kind=='provider_drain' else 'reservoir'
        coverage = data['coverage']; anchor = host.anchors[role]
        m.require(coverage.get('old_process_absent') is True, 'original drain exit unproved')
        validate_span(anchor['log'], coverage['log'], host.root / (role+'-drain.log'))
        if kind=='provider_drain':
            validate_span(anchor['qos'], coverage['qos'], host.root/'provider-drain-qos.jsonl')
            computed = state.verify_provider_drain(context, host.root/'provider-drain.log',
                host.root/'provider-drain-qos.jsonl', coverage, host.root/'final-coupling-journal.json')
        else:
            computed = state.verify_final_snapshot(context, host.root/'reservoir-drain.log', anchor['inventory'], coverage)
            for line in (host.root/'reservoir-drain.log').read_text().splitlines():
                match = re.search(r"snapshot '([^']+)' → (.+)$", line)
                if match:m.require(Path(match[2])==p.STATE_ROOT/(match[1]+'.npz'), 'snapshot target differs')
        m.require(computed==receipt, 'retained drain semantics changed')
        if adoption:
            m.require(host.io.process(host.plan['roles'][role]['identity']['pid']) is None, 'original drain worker survives')
            view=host.inspect_role(role)
            m.require(view.get('held') is not None and not view['listener_pids'], 'drain adoption requires exact holder')
    elif kind=='backup':
        root = host.root/'stopped-backup'; source_inventory = data['source_inventory']
        m.require(data['backup_root']==str(root/'state') and data.get('all_files') is True
                  and data['inventory_sha256']==m.digest(source_inventory), 'backup source binding differs')
        intent=m.read_record(root/'backup-intent.json')
        barrier=intent['barrier']
        m.require(intent['transaction_id']==context['transaction_id'] and intent['status']=='prepared_not_complete'
                  and intent['inventory']==source_inventory and barrier['bindings']==context['bindings']
                  and barrier.get('schema')=='reservoir_maintenance_stopped_barrier_v1'
                  and barrier.get('transaction_id')==context['transaction_id'] and barrier.get('ownership_current') is True
                  and all(barrier.get(role,{}).get(key) is True for role in m.ROLES
                          for key in ('job_absent','old_process_absent','listener_absent'))
                  and source_inventory['root']==str(p.STATE_ROOT) and data['backup_inventory']['root']==str(root/'state')
                  and m.digest(barrier)==data['stopped_barrier_sha256'], 'backup intent differs')
        m.require(state.inventory_tree(root/'state')==data['backup_inventory'], 'retained backup changed')
        from restore_verifier import file_content
        m.require(file_content(source_inventory)==file_content(data['backup_inventory']), 'backup completeness differs')
        from production_host import verify_final_files
        verify_final_files(source_inventory['entries'],host.receipts['final_snapshot']['evidence']['final_handles'],host.signal_times['reservoir'])
        if adoption:
            host._absent('provider');host._absent('reservoir')
            m.require(state.inventory_tree(p.STATE_ROOT)==source_inventory, 'parked source differs from backup')
    elif kind=='startup':
        from production_host import startup_log
        expected=host.receipts['private_restore']['evidence']['expected_startup']
        identity=host.startups['reservoir']['identity']; anchor=host.startups['reservoir']['log_anchor']
        m.require(data['identity']==identity and data['private_restore_sha256']==m.digest(host.receipts['private_restore']), 'startup identity/binding differs')
        validate_span(anchor,data['coverage'],host.root/'reservoir-startup.log')
        actual=startup_log((host.root/'reservoir-startup.log').read_bytes(),expected,host.spec['reservoir_config'])
        m.require(all(data.get(k)==v for k,v in actual.items()), 'startup restoration differs')
        if adoption:ready_identity(host,'reservoir',identity)
    elif kind=='provider_ready':
        identity=host.startups['provider']['identity']
        m.require(data['identity']==identity and data['journal_before_bootstrap']==host.receipts['provider_drain']['evidence']['journal'],
                  'provider-ready prior identity/journal reference differs')
        healthy(data['health'])
        if adoption:ready_identity(host,'provider',identity)
        # No current journal/state comparison: provider may already admit work.
    elif kind!='private_restore':raise m.Refusal('unknown retained witness kind')
    evidence.finish()
    if outer.exists():m.require(m.read_record(outer)==receipt,'canonical outer witness differs')
    elif adoption:m.write_new(outer,receipt)  # Unchanged validated evidence, never a producer replay.
    return receipt


def healthy(health):
    m.require(health.get('ready') is True and health.get('worker',{}).get('phase') in ('ready','generating')
              and health.get('worker',{}).get('last_generation_error') is None and not health.get('error')
              and health.get('reservoir',{}).get('status')=='connected', 'provider semantic readiness failed')


def ready_identity(host, role, identity):
    before=host.inspect_role(role)
    m.require(before.get('process')==identity and before['listener_pids']==[identity['pid']]
              and host.io.process(host.plan['roles'][role]['identity']['pid']) is None,'replacement ownership/listener differs')
    if role=='provider':healthy(host.io.health())
    m.require(host.inspect_role(role)==before,'replacement changed across readiness observation')
    return before


def bootstrap_effect(host,role,pair):
    """Current exact effect proof, never a guessed historical command result."""
    intent=pair['intent'];boundary=intent.get('wall_time_ns')
    m.require(type(boundary) is int and boundary>0,'bootstrap birth boundary missing')
    row=host.io.role(role);ident=m.identity(row.get('process'))
    expected=host.spec['provider_restart_argv'] if role=='provider' else host.plan['roles'][role]['identity']['argv']
    m.require(ident!=host.plan['roles'][role]['identity'] and ident['argv']==expected,'wrong bootstrap replacement')
    # ps lstart is second-granularity local time; allow that one-second rounding
    # only, with original process absent and exact fixed-label/listener evidence.
    try:born=time.mktime(datetime.datetime.strptime(ident['started_at'],'%a %b %d %H:%M:%S %Y').timetuple())
    except (TypeError,ValueError) as error:raise m.Refusal('replacement OS birth time unavailable') from error
    m.require(born+1>=boundary/1e9 and born<=time.time()+1,'replacement predates bootstrap intent')
    ready_identity(host,role,ident)
    if role=='reservoir':
        host._absent('provider')
        # Read the exact current suffix without writing/capturing a second file.
        anchor=intent['log_anchor'];fd,info=p.open_log(anchor['path'])
        try:
            m.require((info.st_dev,info.st_ino)==(anchor['device'],anchor['inode'])
                      and anchor['start_offset']<=info.st_size<=anchor['start_offset']+p.MAX_CAPTURE,'startup log boundary changed')
            prefix,_=p.hash_prefix(fd,anchor['start_offset']);m.require(prefix==anchor['prefix_sha256'],'startup prefix changed')
            remaining=info.st_size-anchor['start_offset'];chunks=[]
            while remaining:
                chunk=os.read(fd,min(1024*1024,remaining));m.require(chunk,'short startup suffix');chunks.append(chunk);remaining-=len(chunk)
            from production_host import startup_log
            proof=startup_log(b''.join(chunks),host.receipts['private_restore']['evidence']['expected_startup'],host.spec['reservoir_config'])
            p.check_log(fd,anchor['path'],info,info.st_size)
        finally:os.close(fd)
    else:
        m.require(host.receipts.get('startup',{}).get('passed') is True,'prior reservoir startup witness required')
        proof={'provider_health':host.io.health()};healthy(proof['provider_health'])
    return {'identity':ident}, {'intent':intent,'observed':row,'startup':proof,'birth_wall_seconds':born,
                              'historical_command_return_inferred':False}


def reconcile(host,action_id,checkpoint,inspection):
    session=require_session(host)
    m.require(checkpoint.get('action_id')==action_id,'reconciliation is not the current finite action')
    m.require(host._recovery_validated=={'checkpoint':checkpoint,'inspection':inspection,
              'ledger_head':checkpoint['recovery_head_sha256']},'reconciliation lacks revalidated retained checkpoint')
    from recovery_ledger import read_ledger
    ledger=read_ledger(host.root,host.plan);claim=ledger['records'][-1] if ledger['records'] else {}
    m.require(claim.get('type')=='reconciliation_claim' and claim.get('action_id')==action_id
              and claim.get('checkpoint_sha256')==m.digest(checkpoint) and claim.get('inspection_sha256')==m.digest(inspection)
              and claim.get('executor_session')==session.begin,'reconciliation lacks exact current claim')
    operation,_,role=action_id.partition(':');role=role or None
    m.require(claim.get('context')==context_for(host,role if operation=='witness' else None),
              'reconciliation claim context differs from accepted prerequisites')
    from production_recovery_inspect import Evidence,validate_retained,host_records,controller_records,snapshot,anchors
    evidence=Evidence();plan,_,variants=validate_retained(host.spec,host.root,evidence)
    m.require(snapshot(host.io,plan)==inspection['observed_after'], 'live ownership changed after reconciliation claim')
    m.require(variants==inspection['installed_variants'] and anchors(host.root,plan,evidence)==checkpoint['anchors'],
              'retained phase/anchors changed after reconciliation claim')
    control=controller_records(host.root/'controller',plan,evidence)
    history=host_records(host.root,plan,control,evidence,ledger['records'])
    m.require(history['events']==inspection['host_operations']['events'] and control['last_sha256']==checkpoint['controller_head_sha256'],
              'execution history changed after reconciliation claim')
    pair_key={'create_hold':'hold_intent','arm_next_spawn':'debug_intent','signal_stop':'signal_intent',
              'remove_held':'held_bootout_intent','install_repair':'install_intent',
              'apply_provider_restart':'provider_snapshot_plist_intent','bootstrap_original':'bootstrap_intent',
              'release_hold':'hold_release_intent'}.get(operation)
    pair=history['pairs'].get((pair_key,role)) if pair_key else None
    result=None;proof={}
    if operation=='preflight':
        result=m.preflight(plan,host)
        proof={'current_read_only_preflight':True,'historical_return_inferred':False}
    elif operation=='completed':
        for target in m.ROLES:
            ready_identity(host,target,host.startups[target]['identity'])
            m.require(not os.path.lexists(host.control(target)/'hold.json'),'guard release is not complete')
        host._ownership()
        result={'schema':'reservoir_maintenance_result_v1','status':'verified','transaction_id':plan['transaction_id'],
                'plan_sha256':m.digest(plan),**{target:host.startups[target]['identity'] for target in m.ROLES},
                'force_used':False,'runtime_state_rollback':False,'execution_scope':host.scope,
                'production_qualified':False,'no_missed_input_claimed':False,
                'witness_hashes':{kind:m.digest(receipt) for kind,receipt in host.receipts.items()}}
        proof={'current_completion_observed':True,'historical_return_inferred':False}
    elif operation=='witness':
        result=validate_existing_witness(host,role,context_for(host,role),adoption=True)
        proof={'canonical_witness':{'path':str(host.root/(role+'-witness.json')),
                                    'sha256':p.sha(host.root/(role+'-witness.json'))},'producer_replayed':False}
    elif operation in ('arm_next_spawn','signal_stop','held_verified'):
        m.require(operation=='held_verified' or pair is not None or
                  (operation=='signal_stop' and role in checkpoint.get('held_effects',{})),
                  'no retained operation intent or already-adopted holder effect')
        view=host.inspect_role(role);old=plan['roles'][role]['identity'];held=view.get('held')
        recorded=pair.get('result') if pair else None
        if operation=='arm_next_spawn' and recorded is not None and recorded.get('returncode')==0 and view.get('process')==old:
            host.env.assert_hold(host.control(role),host.guards[role])
            m.require(view['listener_pids']==[old['pid']], 'armed original listener differs')
            result={'armed':True}
            proof={'recorded_result':recorded,'observed':view,'historical_result_inferred':False}
        elif operation=='signal_stop' and recorded is not None and view.get('process')==old:
            host.env.assert_hold(host.control(role),host.guards[role])
            m.require(recorded.get('identity')==old, 'recorded signal target differs')
            result={'signal':'SIGTERM','identity':old,'force_used':False}
            proof={'recorded_result':recorded,'observed':view,'historical_result_inferred':False}
        else:
            m.require(held is not None and not view['listener_pids'] and host.io.process(old['pid']) is None,
                      'exact owned holder and original exit required')
            result={'identity':held['identity']} if operation=='held_verified' else {
                'effect':'owned_holder_observed','identity':held['identity'],'original_identity':old,'guard':host.guards[role]}
            proof={'observed':view,'historical_signal_or_debug_return_inferred':False}
    elif operation=='create_hold':
        m.require(pair is not None,'no retained hold intent');host.env.assert_hold(host.control(role),pair['intent']['guard'])
        result={'guard':pair['intent']['guard']};proof={'existing_guard':result['guard']}
    elif operation in ('remove_held','job_absent'):
        m.require(operation=='job_absent' or pair is not None,'no retained removal intent')
        host._absent(role)
        if pair:
            ident=pair['intent']['identity'];m.require(host.io.process(ident['pid']) is None,'removed holder survives')
            result={'removed_identity':ident}
        else:result={'absent':True}
        proof={'fixed_job_old_worker_holder_and_listener_absent':True}
    elif operation=='install_repair':
        m.require(pair is not None and host.receipts.get('private_restore',{}).get('passed') is True,'repair adoption prerequisites missing')
        host._absent('provider');host._absent('reservoir')
        m.require(variants['reservoir']['source']=='candidate','intended repair bytes not installed')
        import restore_verifier as v
        v.validate_restore_witness(Path(host.receipts['private_restore']['evidence']['work_root'])/'restore-witness.json',
                                  context_for(host,'private_restore'),host.receipts['backup'])
        m.require(state.inventory_tree(p.STATE_ROOT)==host.receipts['backup']['evidence']['source_inventory'],'parked state changed before repair adoption')
        result={'source_sha256':plan['input_hashes']['reservoir_candidate']};proof={'installed_variant':variants['reservoir']}
    elif operation=='apply_provider_restart':
        m.require(pair is not None and host.receipts.get('startup',{}).get('passed') is True,'pin adoption prerequisites missing')
        host._absent('provider');ready_identity(host,'reservoir',host.startups['reservoir']['identity'])
        m.require(variants['provider']['plist']=='pinned' and p.sha(p.JOURNAL)==host.receipts['provider_drain']['evidence']['journal']['sha256'],
                  'provider pin/current pre-admission journal differs')
        result={'provider_settings_sha256':plan['input_hashes']['provider_restart_settings'],
                'provider_restart_argv':host.spec['provider_restart_argv']};proof={'installed_variant':variants['provider']}
    elif operation=='bootstrap_original':
        m.require(pair is not None,'no retained bootstrap intent')
        m.require(variants[role]['source']==('candidate' if role=='reservoir' else 'original')
                  and variants[role]['plist']==('pinned' if role=='provider' else 'original'),'bootstrap installed inputs differ')
        result,proof=bootstrap_effect(host,role,pair)
    elif operation=='release_hold':
        m.require(pair is not None and host.receipts.get('provider_ready',{}).get('passed') is True,'release adoption prerequisites missing')
        m.require(not os.path.lexists(host.control(role)/'hold.json'),'hold release not observed')
        for target in m.ROLES:ready_identity(host,target,host.startups[target]['identity'])
        result={'released':True};proof={'prior_guard':pair['intent']['guard'],'guard_absent_now':True}
    else:raise m.Refusal('no exact-effect reconciliation for action: '+action_id)
    m.require(snapshot(host.io,plan)==inspection['observed_after'], 'live ownership changed during reconciliation')
    evidence.finish()
    return {'schema':'reservoir_recovery_reconciliation_v1','action_id':action_id,
            'transaction_id':plan['transaction_id'],'plan_sha256':m.digest(plan),'checkpoint_sha256':m.digest(checkpoint),
            'inspection_sha256':m.digest(inspection),'passed':True,'result':result,'evidence':proof}
