#!/usr/bin/env python3
"""One-use abort of the exact failed synthetic fixture. Default: no action."""
from __future__ import annotations
import argparse
import datetime
import json
import os
from pathlib import Path
import sys
import time

import abort_evidence as e
from abort_native import NativeHost, sync_directory, write_new


class Ledger:
    def __init__(self,owner):
        self.owner=owner;self.rows=[];self.actions=set();self.previous=e.digest(owner)

    def record(self,kind,**values):
        row={'schema':'owned_fixture02_abort_event_v1','sequence':len(self.rows)+1,
             'previous_sha256':self.previous,'type':kind,'wall_time_ns':time.time_ns(),
             'monotonic':time.monotonic(),'executor_identity':self.owner['executor_identity'],**values}
        if kind=='intent':
            e.require(row['action'] not in self.actions,'abort operation already attempted');self.actions.add(row['action'])
        write_new(e.ABORT/f'event-{len(self.rows)+1:05d}.json',row)
        self.rows.append(row);self.previous=e.digest(row);return row


def exact_holder(view,expected=None):
    e.require(view['loaded'] and view['held'] and not view['port_pids'] and view['identity'] is not None,
              'exact no-listener holder not observed')
    if expected is not None:e.require(view['identity']==expected,'holder identity changed')
    return view['identity']


def initial_views(host,t):
    provider=host.inspect('provider');reservoir=host.inspect('reservoir')
    exact_holder(provider,t['provider_holder'])
    e.require(reservoir['loaded'] and not reservoir['held'] and reservoir['identity']==t['workers']['reservoir']
              and reservoir['port_pids']==[t['workers']['reservoir']['pid']], 'exact original reservoir not ready')
    return {'provider':provider,'reservoir':reservoir}


def wait_for(host,predicate,label,seconds):
    deadline=host.monotonic()+seconds
    while True:
        e.require(host.monotonic()<deadline,label+' timed out; no escalation or retry')
        value=predicate()
        now=host.monotonic()
        e.require(now<deadline,label+' timed out; no escalation or retry')
        if value:return value
        host.sleep(min(.25,deadline-now))


def sequence(host,t,record):
    """Finite Host protocol; tests use an inert Host, never native commands."""
    e.require(host.scope=='failed_owned_fixture_abort_only','abort Host scope differs')
    record('initial_verified',proof=host.verify(),roles=initial_views(host,t))
    def action(name,call,**values):
        claim=record('intent',action=name,**values)
        result=call();record('result',action=name,claim_sha256=e.digest(claim),result=result)
        return claim,result
    action('create_reservoir_guard',lambda:host.create_guard(record))
    host.verify();initial_views(host,t)
    _,armed=action('debug_reservoir',lambda:host.debug(record))
    e.require(armed['returncode']==0 and armed['naturally_reaped'] is True,'debug lacks successful natural return')
    host.verify();initial_views(host,t)
    signal_intent,_=action('signal_reservoir',lambda:host.signal_reservoir(record),identity=t['workers']['reservoir'],signal='SIGTERM')
    def held_after_drain():
        exact_holder(host.inspect('provider'),t['provider_holder'])
        view=host.inspect('reservoir')
        e.require(view['loaded'],'reservoir unexpectedly unloaded while draining')
        if view['held']:
            e.require(host.process(t['workers']['reservoir']['pid']) is None,'old reservoir survives its holder')
            return view
        current=view['identity']
        e.require(current is None or current==t['workers']['reservoir']
                  or view.get('holder_pending') is True,'foreign replacement during settling')
        return None
    reservoir=wait_for(host,held_after_drain,'cooperative reservoir save/hold',90)
    holders={'provider':t['provider_holder'],'reservoir':exact_holder(reservoir)}
    saved=host.verify(saved=True);host.verify_signal_boundary(signal_intent)
    record('durable_save_verified',proof=saved,signal_intent_sha256=e.digest(signal_intent),holders=holders,
           saved_artifacts=host.capture_saved())
    provider_released=False
    for role in ('provider','reservoir'):
        host.verify(saved=True,provider_released=provider_released)
        exact_holder(host.inspect(role),holders[role])
        action('remove_'+role,lambda role=role:host.remove_held(role,holders[role],record),identity=holders[role])
        def absent(role=role):
            loaded,pid=host.job(role)
            if loaded:
                e.require(pid in (None,holders[role]['pid']),'foreign PID appeared after holder removal')
                return False
            e.require(pid is None,'unloaded job has PID')
            return host.process(holders[role]['pid']) is None and not host.port_pids(role)
        wait_for(host,absent,'held '+role+' job/PID/listener absence',15)
        action('release_'+role+'_guard',lambda role=role:host.release_guard(role,holders[role],record))
        if role=='provider':provider_released=True
    proof=host.verify(saved=True,provider_released=True)
    for role,ident in holders.items():
        e.require(host.job(role)==(False,None) and host.process(ident['pid']) is None and not host.port_pids(role), 'owned holder/job/listener remains')
    e.require(host.process(t['workers']['reservoir']['pid']) is None,'old reservoir survives')
    final=host.final_evidence()
    result={'schema':'owned_fixture02_abort_result_v1','passed':True,'cleanup_only':True,
            'original_fixture_passed':False,'production_qualified':False,'original_release':t['release'],
            'original_snapshot':t['snapshot'],'state':e.STATE,'proof':proof,'final':final,
            'force_used':False,'stateful_worker_bootout':False,'provider_replayed':False,
            'resumed_original_main':False,'abort_executor_absence_requires_independent_closeout':True}
    record('completed',result=result);return result


def validate_authorization(path,t):
    e.require(Path(path)==e.HERE/'evidence/abort-release-01.json','fixed reviewed abort release required')
    value=e.read(path);artifacts=e.source_artifacts()
    e.require(value.get('schema')=='owned_fixture02_abort_release_v1' and value.get('approved') is True
              and value.get('execution_authorized') is True and value.get('scope')=='exact_failed_fixture_abort_only'
              and value.get('production_qualified') is False and value['artifacts']==artifacts
              and value['target']==e.reference(e.HERE/'target.json') and value['original_release']==t['release']
              and value['snapshot']==t['snapshot'] and value['design']==t['design'],'abort release/source binding differs')
    until=datetime.datetime.fromisoformat(value['not_after'])
    e.require(until.tzinfo is not None and datetime.datetime.now(datetime.timezone.utc)<until,'abort release expired')
    cpu=e.checked(value['cpu_receipt']);review=e.checked(value['independent_review'])
    e.require(cpu.get('passed') is True and cpu.get('artifacts')==artifacts
              and review.get('approved') is True and review.get('artifacts')==artifacts
              and review.get('cpu_receipt')==value['cpu_receipt'] and review.get('scope')=='exact_failed_fixture_abort_only',
              'exact source/test/independent review required')
    return {'path':str(path),'sha256':e.sha(Path(path))},artifacts


def preflight(host,t,release,*,claimed=False):
    if claimed:
        attempt=e.read(e.ABORT/'attempt.json')
        e.require(attempt['executor_pid']==os.getpid() and attempt['original_release']==t['release'], 'foreign abort attempt')
    else:e.require(not os.path.lexists(e.ABORT),'abort root already exists; one-use/no resume')
    e.require(os.getuid()==501 and os.geteuid()==501,'ordinary UID501 required')
    e.require(str(Path(sys.executable).resolve(strict=True)) in t['python_files'],'use exact reviewed Python family')
    sources=e.verify_sources(t,release)
    proof=host.verify();views=initial_views(host,t)
    return {'schema':'owned_fixture02_abort_preflight_v1','passed':True,'sources':sources,'proof':proof,'roles':views,
            'target':e.reference(e.HERE/'target.json'),'service_actions':False}


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    mode=parser.add_mutually_exclusive_group();mode.add_argument('--check',action='store_true');mode.add_argument('--execute',action='store_true')
    parser.add_argument('--reviewed-release',type=Path)
    args=parser.parse_args()
    if not args.check and not args.execute:
        print(json.dumps({'status':'not_executed','scope':'fixed failed fixture abort only','default':'no queries or writes'}));return 0
    t,snapshot,release=e.target();host=NativeHost(t,snapshot,release)
    authorization=artifacts=None
    if args.execute:authorization,artifacts=validate_authorization(args.reviewed_release,t)
    with host.lock() as lock:
        if args.check:
            try:
                checked=preflight(host,t,release)
                print(json.dumps({**checked,'queries':host.queries},sort_keys=True));return 0
            except BaseException as error:
                print(json.dumps({'passed':False,'error_type':type(error).__name__,'error':str(error),
                                  'queries':host.queries,'service_actions':False},sort_keys=True))
                raise
        e.require(os.getuid()==501 and os.geteuid()==501 and str(Path(sys.executable).resolve(strict=True)) in t['python_files'],
                  'ordinary UID501 with exact reviewed Python required')
        e.require(sys.stdin.isatty(),'interactive Terminal stdin required')
        e.require(not os.path.lexists(e.ABORT),'abort root already exists; one-use/no resume')
        e.require(e.source_artifacts()==artifacts and e.reference(Path(authorization['path']))==authorization,'abort authorization drift')
        e.ABORT.mkdir(mode=0o700);sync_directory(e.ABORT.parent)
        write_new(e.ABORT/'attempt.json',{'schema':'owned_fixture02_abort_attempt_v1','executor_pid':os.getpid(),
                  'original_release':t['release'],'original_snapshot':t['snapshot'],'authorization':authorization,
                  'artifacts':artifacts,'execution_lock':lock,'started_wall_time_ns':time.time_ns(),'retry_permitted':False})
        ledger=None
        try:
            checked=preflight(host,t,release,claimed=True)
            e.require(e.source_artifacts()==artifacts and e.reference(Path(authorization['path']))==authorization,'abort authorization drift')
            owner={'schema':'owned_fixture02_abort_owner_v1','scope':'exact_failed_fixture_abort_only',
                   'original_release':t['release'],'original_snapshot':t['snapshot'],'target':e.reference(e.HERE/'target.json'),
                   'authorization':authorization,'artifacts':artifacts,'executor_identity':host.executor(),
                   'host_binding':t['host_binding'],'execution_lock':lock,'preflight':checked,'original_owner_rewritten':False}
            write_new(e.ABORT/'owner.json',owner);ledger=Ledger(owner)
            host.activate(owner);result=sequence(host,t,ledger.record)
            write_new(e.ABORT/'result.json',result)
        except BaseException as error:
            if ledger is not None:
                ledger.record('failed',error_type=type(error).__name__,error=str(error)[:1000],
                              retry_permitted=False,cleanup='none; retain owned jobs/guards and action children')
            write_new(e.ABORT/'failure.json',{'error_type':type(error).__name__,'error':str(error)[:1000],
                      'original_fixture_passed':False,'production_qualified':False,'retry_permitted':False})
            raise
        finally:
            write_new(e.ABORT/'raw-queries.json',{'schema':'owned_fixture02_abort_queries_v1','queries':host.queries})
    print(json.dumps({'passed':True,'receipt':str(e.ABORT/'result.json'),'original_fixture_passed':False}));return 0


if __name__=='__main__':
    raise SystemExit(main())
