"""Two fixed parent readiness stages, before sealed recovery inspection.

No actions, producer retries or authoritative snapshot refresh after child claim.
"""
from __future__ import annotations
import copy
import math
from pathlib import Path
import time

import owned_fixture_records as r


def selected(step):
    return step['kind']=='resume_one' and step['mode']=='reconcile' and step['action_id'] in (
        'held_verified:provider','held_verified:reservoir')


def basis(host,step,release):
    """Retained success is used only to wait; full core validation follows ready."""
    from owned_fixture_retained import history
    role=step['action_id'].split(':')[1];old=host.plan['roles'][role]['identity'];hist=history(host)
    debug=hist['pairs'].get(('debug_intent',role));signal=hist['pairs'].get(('signal_intent',role))
    r.require(debug and signal and debug['result'] and signal['result']
              and debug['intent']['identity']==signal['intent']['identity']==signal['result']['identity']==old
              and debug['result']['returncode']==0 and signal['intent']['signal']==signal['result']['signal']=='SIGTERM',
              'readiness lacks exact successful debug/signal')
    ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
    last=ledger['records'][-1] if ledger['records'] else {}
    r.require(last.get('type')=='reconciliation_result' and last.get('action_id')=='signal_stop:'+role,
              'readiness does not follow accepted signal reconciliation')
    steps=release['executor_plan'];index=steps.index(step);previous=steps[index-1]
    r.require(previous['action_id']=='signal_stop:'+role and previous['mode']=='reconcile','wrong sealed predecessor')
    exit_path=host.root/'executors'/previous['id']/'exit.json';exit_record=r.read(exit_path)
    r.require(exit_record['reaped'] is True and exit_record['returncode']==0 and exit_record['timed_out'] is False
              and host.io.process(exit_record['pid']) is None,'previous signal executor not reaped/absent')
    host.armed.add((role,old['pid'],old['started_at']));host.signalled.add((role,old['pid'],old['started_at']))
    def refs(pair):
        return {kind:next(ref for path,ref in hist['refs'].items() if r.read(path)==pair[kind]) for kind in ('intent','result')}
    return role,old,{'prior_debug':refs(debug),'prior_signal':refs(signal),'previous_exit':r.reference(exit_path)}


def stable_heads(host):
    from owned_fixture_retained import history
    journal=host.m.Journal(host.root/'controller');ledger=host.core['recovery_ledger'].read_ledger(host.root,host.plan)
    return {'controller_head_sha256':journal.previous,'host_operations_sha256':r.digest(history(host)['events']),
            'recovery_head_sha256':ledger['head_sha256']}


def observe_other(host,role):
    other='reservoir' if role=='provider' else 'provider';view=host.inspect_role(other);old=host.plan['roles'][other]['identity']
    if role=='provider':r.require(view['job_loaded'] and view['process']==old and not view['held']
                                 and view['listener_pids']==[old['pid']],'other original role changed')
    else:r.require(not view['job_loaded'] and view['process'] is None and not view['listener_pids']
                   and host.io.process(old['pid']) is None,'other parked role changed')
    return view


def prepare(operation,host,step,parent_claim):
    """Return one sealed directory plus the unchanged original phase deadline."""
    import owned_fixture_coordinator as c
    from owned_recovery_host import readiness_transition
    r.require(selected(step),'unapproved readiness phase')
    phase_start=host.monotonic();phase_deadline=phase_start+300
    deadline=min(phase_deadline,phase_start+host.plan['timeout_s'])
    r.require(host.plan['timeout_s']==90,'readiness cannot change original90second bound')
    folder=host.root/'readiness'
    if not folder.exists():folder.mkdir(mode=0o700)
    r.directory(folder);path=folder/(step['id']+'.json');r.require(not path.exists(),'readiness phase already attempted')
    samples=[];record=None
    with operation.core['transaction_execution'].ExecutionSession(host.root,host.plan,host.executor_identity(),'resume_one') as session:
        try:
            release=c.owned_release(host,operation.release_ref);role,old,prior=basis(host,step,release)
            heads=stable_heads(host);guard=host.guard();pending_identity=None
            start_ns=time.time_ns()
            while True:
                begin=host.monotonic();r.require(begin<deadline,'readiness acceptance deadline expired')
                release=c.owned_release(host,operation.release_ref)
                r.require(stable_heads(host)==heads,'readiness history changed')
                qstart=len(host.io.observations)
                target,old_seen=readiness_transition(host,role,old,guard);other=observe_other(host,role)
                if target['process'] not in (None,old):
                    found={'pid':target['process']['pid'],'started_at':target['process']['started_at']}
                    if pending_identity is None:pending_identity=found
                    r.require(pending_identity==found,'pending holder identity changed')
                end=host.monotonic()
                samples.append({'started_monotonic':begin,'completed_monotonic':end,'observed_wall_time_ns':time.time_ns(),
                    'target':target,'old_pid_observation':old_seen,'other_role':other,'guard':guard,
                    'queries':copy.deepcopy(host.io.observations[qstart:])})
                r.require(len(samples)<=901 and end<deadline,'readiness acceptance deadline expired or sample bound exceeded')
                if target['held']:
                    r.require(old_seen is None and not target['listener_pids'],'ready holder retains original/listener');break
                host.sleep(min(.1,deadline-end))
            r.require(stable_heads(host)==heads,'readiness history changed after holder observation')
            inspection=host.recovery_inspection(host.m.Journal(host.root/'controller'))
            ledger=operation.core['recovery_ledger'].read_ledger(host.root,host.plan)
            checkpoint=operation.core['recovery_phases'].checkpoint_from_inspection(host.spec,inspection,ledger)
            r.require(checkpoint['action_id']==step['action_id'] and inspection['roles'][role]['observed']==target
                      and all(checkpoint[key]==value for key,value in heads.items()),'authoritative ready inspection differs')
            operation.core['production_recovery_driver'].validate_phase_observation(host.plan,checkpoint,inspection,reconcile=True)
            expected=operation.core['production_recovery_driver'].expected_heads(checkpoint)
            r.require(host.monotonic()<deadline,'authoritative readiness capture exceeded acceptance deadline')
            inspection_ref=r.write(folder/(step['id']+'-inspection.json'),inspection)
            parent_identity=host.executor_identity()
            r.require(host.monotonic()<deadline,'readiness identity query exceeded acceptance deadline')
            record={'schema':'owned_fixture_holder_readiness_v1','step':step,'release':operation.release_ref,'root':str(host.root),
                'parent_claim':parent_claim,'parent_identity':parent_identity,'executor_session':session.begin,**prior,
                'stable_heads':heads,'phase_started_monotonic':phase_start,'phase_deadline_monotonic':phase_deadline,
                'readiness_deadline_monotonic':deadline,'started_wall_time_ns':start_ns,'completed_wall_time_ns':time.time_ns(),
                'samples':samples,'final_inspection':inspection_ref,'expected_heads':expected,'ready_identity':target['process'],
                'guard':guard,'source_artifacts_sha256':r.digest(release['artifacts']),'coordination':release['coordination']}
            readiness=r.write(path,record)
            directory=c.prepare_executor(host.root,operation.release_ref,step['id'],host,expected_heads=expected,readiness=readiness)
            r.require(host.monotonic()<phase_deadline,'readiness preparation exhausted original phase budget')
        except BaseException as error:
            if record is None:r.write(folder/(step['id']+'-refused.json'),{'step':step,'parent_claim':parent_claim,
                'samples':samples,'error_type':type(error).__name__,'error':str(error)[:1000],'retry_permitted':False})
            raise
    remaining=math.floor(phase_deadline-host.monotonic())
    r.require(remaining>=1,'original executor phase budget exhausted before launch')
    return directory,readiness,phase_deadline


def validate_request(host,request):
    """The child validates an old sealed record; it never waits or refreshes it."""
    step=request['step'];r.require(selected(step)==('readiness' in request),'readiness reference missing or unplanned')
    if not selected(step):return
    ref=request['readiness'];r.require(ref['path']==str(host.root/'readiness'/(step['id']+'.json')),'readiness outside selected step')
    value=r.check_reference(ref)
    r.require(value['schema']=='owned_fixture_holder_readiness_v1' and value['step']==step
              and value['release']==request['release'] and value['root']==str(host.root)
              and value['parent_identity']==request['coordinator_identity'] and value['expected_heads']==request['expected_heads'],
              'sealed readiness binding differs')
    r.check_reference(value['parent_claim']);inspection=r.check_reference(value['final_inspection'])
    r.require(r.digest(inspection)==request['expected_heads']['inspection_sha256'], 'readiness inspection digest differs')
