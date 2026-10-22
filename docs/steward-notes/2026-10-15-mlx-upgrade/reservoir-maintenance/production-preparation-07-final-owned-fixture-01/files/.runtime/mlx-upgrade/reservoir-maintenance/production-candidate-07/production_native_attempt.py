"""Explicit one-use native observation attempts and session-bound effect sinks.

Reservation writes private evidence only. It grants no service-control authority.
Planned boot/parent fields remain unverified until exact read-only queries finish.
"""
from __future__ import annotations
import hashlib
import os
from pathlib import Path
import re
import sys
import time

import production_native as native
import reservoir_maintenance as m

HERE=Path(__file__).resolve().parent
OBSERVATIONS=HERE.parent/'production-native-observations'


def ref(path):
    return {'path':str(path),'sha256':hashlib.sha256(Path(path).read_bytes()).hexdigest()}


def private(path):
    info=path.lstat()
    m.require(path==path.resolve(strict=True) and path.is_dir() and info.st_uid==os.getuid()
              and not info.st_mode & 0o077,'native evidence directory must be canonical/private/owned')


def invocation(spec, env):
    """Preserve the pinned interpreter's original options and exact script tail."""
    original=getattr(sys,'orig_argv',None);tail=list(sys.argv)
    m.require(isinstance(original,list) and len(original)>len(tail) and tail
              and all(isinstance(value,str) and value for value in original+tail),
              'complete original interpreter invocation required')
    split=len(original)-len(tail);options=original[1:split]
    m.require(original[split:]==tail and len(set(options))==len(options)
              and all(value in ('-I','-B') for value in options),'interpreter options/script tail differ')
    script=Path(tail[0]);launcher=Path(original[0])
    m.require(script in {HERE/name for name in ('production_entrypoint.py','production_preflight.py','production_recovery_inspect.py')}
              and script==script.resolve(strict=True) and launcher.is_absolute(), 'fixed entry script/launcher required')
    family=env.interpreter_paths(launcher)
    m.require(Path(sys.executable).resolve(strict=True) in family
              and all(spec['immutable_files'].get(str(path))==hashlib.sha256(path.read_bytes()).hexdigest() for path in family),
              'original interpreter invocation family differs')
    return list(original)


def reserve(preparation_path, preparation_sha256, spec, mode, name, *, io_class):
    m.require(Path(native.__file__).resolve()==HERE/'production_native.py','native helper origin differs')
    m.require(isinstance(name,str) and re.fullmatch('[a-z0-9][a-z0-9-]{0,63}',name),'explicit native attempt name required')
    m.require(mode in ('check','inspect','fresh','resume-execute','resume-reconcile'),'native attempt mode differs')
    m.require(os.getuid()==os.geteuid()>0,'ordinary-user native evidence required')
    path=Path(preparation_path)
    m.require(path==path.resolve(strict=True) and hashlib.sha256(path.read_bytes()).hexdigest()==preparation_sha256,
              'native preparation changed')
    sources={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in HERE.glob('*.py')}
    m.require(all(spec['immutable_files'].get(p)==sha for p,sha in sources.items()),'complete native source closure unbound')
    import production_preflight as p
    env=p.load_hold();expected_argv=invocation(spec,env)
    OBSERVATIONS.mkdir(mode=0o700,exist_ok=True);private(OBSERVATIONS)
    root=OBSERVATIONS/name;root.mkdir(mode=0o700)
    owner={'schema':'production_native_observation_owner_v1','root':str(root),'name':name,
        'preparation':{'path':str(path),'sha256':preparation_sha256},'mode':mode,'source_artifacts':sources,
        'uid':os.getuid(),'pid':os.getpid(),'expected_argv':expected_argv,
        'planned_host_binding':spec['plan']['host_binding'],'identity_verified':False,'created_wall_time_ns':time.time_ns()}
    m.write_new(root/'owner.json',owner)
    recorder=native.Recorder(root,owner);io=io_class(recorder=recorder)
    actual=io.process(os.getpid());host=io.host()
    m.require(actual is not None and actual['pid']==owner['pid']
              and env.command_matches(actual['argv'],owner['expected_argv'])
              and host==owner['planned_host_binding'],'native parent identity/boot verification failed; retain attempt')
    verification={'schema':'production_native_observation_verified_v1','owner':ref(root/'owner.json'),
        'identity':actual,'host_binding':host,'verified_wall_time_ns':time.time_ns(),'control_authority':False}
    m.write_new(root/'verified.json',verification)
    io.native_verification=ref(root/'verified.json')
    return io


def mutation_recorder(host):
    from transaction_execution import require_active
    session=require_active(host.root,host.plan)
    query=getattr(host.io,'recorder',None);verification=getattr(host.io,'native_verification',None)
    m.require(type(query) is native.Recorder and verification is not None,'verified native query attempt required')
    query.check()
    verified=m.read_record(Path(verification['path']))
    m.require(ref(Path(verification['path']))==verification and verified['owner']==ref(query.root/'owner.json')
              and verified['identity']==session.executor and verified['host_binding']==host.plan['host_binding'],
              'native query attempt is not this verified executor/session')
    parent=host.root/'native-commands'
    parent.mkdir(mode=0o700,exist_ok=True);private(parent)
    root=parent/query.owner['name']
    saved=getattr(host,'_native_recorder',None)
    if saved is not None:
        m.require(saved.root==root and saved.owner['executor_session']==session.begin,'native effect session changed')
        saved.check();return saved
    # Never adopt or append to an existing command attempt after interruption.
    root.mkdir(mode=0o700)
    owner={'schema':'production_native_effect_owner_v1','uid':os.getuid(),'pid':os.getpid(),
        'root':str(root),'transaction_id':host.plan['transaction_id'],'plan_sha256':m.digest(host.plan),
        'observation_verification':verification,'executor_session':session.begin,'created_wall_time_ns':time.time_ns()}
    m.write_new(root/'owner.json',owner)
    host._native_recorder=native.Recorder(root,owner)
    return host._native_recorder
