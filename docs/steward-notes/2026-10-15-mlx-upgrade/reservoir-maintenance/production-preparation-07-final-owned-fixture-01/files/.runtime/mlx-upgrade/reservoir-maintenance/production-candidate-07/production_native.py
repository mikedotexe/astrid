"""Finite native command boundary. Definitions only; no implicit attempt creation.

Popen ownership is recorded before waiting. Timeout, interruption and publication
failure retain uncertainty; this module never signals, retries or closes a live
child through a Popen context manager. A caller must supply its bound evidence
recorder before even a read-only query can spawn.
"""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import stat
import re
import subprocess
import time
from types import SimpleNamespace

import reservoir_maintenance as m

MAX_OUTPUT = 4 * 1024 * 1024
MAX_COMMANDS = 12000


def readonly_argv(argv, *, uid, labels, ports, inventory_argv):
    a = tuple(argv)
    pid = lambda s: isinstance(s,str) and re.fullmatch('[1-9][0-9]*',s) is not None and int(s)>1
    return (a == ('/usr/sbin/sysctl','-n','kern.bootsessionuuid')
        or a == ('/usr/bin/sw_vers','-productVersion')
        or len(a)==5 and a[:4]==('/bin/ps','-o','lstart=','-p') and pid(a[4])
        or len(a)==6 and a[:5]==('/bin/ps','-ww','-o','command=','-p') and pid(a[5])
        or len(a)==3 and a[:2]==('/bin/launchctl','print') and a[2] in {f'gui/{uid}/{v}' for v in labels.values()}
        or len(a)==5 and a[:2]==('/usr/sbin/lsof','-nP') and a[2] in {f'-iTCP:{p}' for p in ports.values()} and a[3:]==('-sTCP:LISTEN','-Fp')
        or len(a)==8 and a[:4]==('/usr/sbin/lsof','-nP','-a','-p') and pid(a[4]) and a[5:]==('-iTCP','-sTCP:LISTEN','-Fp')
        or a == tuple(inventory_argv))


def mutation_argv(kind, argv, *, uid, role, labels, plists, holder_argv):
    m.require(role in labels, 'unknown fixed production role')
    target = f'gui/{uid}/{labels[role]}'
    wanted = {
        'debug': ['/usr/bin/sudo','--','/bin/launchctl','debug',target,'--program',holder_argv[0],'--',*holder_argv[1:]],
        'bootout': ['/bin/launchctl','bootout',target],
        'bootstrap': ['/bin/launchctl','bootstrap',f'gui/{uid}',str(plists[role])],
    }
    m.require(kind in wanted and list(argv)==wanted[kind], 'nonfixed native mutation refused')


class Recorder:
    """One externally reserved ordinary-user attempt, no takeover or auto-resume.

    The owner record must already exist and be supplied by the enclosing finite
    entrypoint/transaction. This object never creates a root or invents owner
    authority. Reattaching a nonempty command directory is deliberately blocked.
    """
    def __init__(self, root, owner):
        self.root=Path(root); self.owner=owner; self.count=0; self.failed=False
        m.require(self.root.is_absolute() and self.root==self.root.resolve(strict=True), 'canonical native attempt required')
        info=self.root.lstat()
        m.require(stat.S_ISDIR(info.st_mode) and info.st_uid==os.getuid() and not info.st_mode & 0o077,
                  'private ordinary-user native attempt required')
        m.require(m.read_record(self.root/'owner.json')==owner and owner.get('pid')==os.getpid()
                  and owner.get('uid')==os.getuid() and os.getuid()==os.geteuid()>0,
                  'native attempt owner differs')
        m.require(set(self.root.iterdir())=={self.root/'owner.json'}, 'retained native attempt cannot be replayed')

    def check(self):
        m.require(not self.failed and m.read_record(self.root/'owner.json')==self.owner,
                  'native attempt failed or owner changed')

    def write(self, path, value):
        m.write_new(path,value)
        return {'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}

    def run(self, argv, *, kind, terminal=False, timeout=20):
        self.check()
        m.require(timeout==(180 if terminal else 20) and (not terminal or kind=='debug'), 'native deadline changed')
        m.require(isinstance(argv,list) and argv and all(isinstance(s,str) and s for s in argv), 'exact argv required')
        self.failed=True  # Any partial directory/intent/output publication blocks reuse.
        self.count+=1; m.require(self.count<=MAX_COMMANDS,'native command budget exhausted')
        base=self.root/f'command-{self.count:05d}'; base.mkdir(mode=0o700)
        child=None; created=None; result=None; streams=[]; waited=False; observed_rc=None
        intent=self.write(base/'intent.json',{'schema':'production_native_intent_v1','owner':self.owner,
            'command_id':self.count,'argv':argv,'kind':kind,'terminal':terminal,'timeout_s':timeout,
            'wall_time_ns':time.time_ns(),'monotonic':time.monotonic(),'retry_permitted':False})
        try:
            # Files avoid unbounded in-memory buffering and pipe deadlock. They
            # remain evidence if a child outlives this bounded observation.
            if not terminal:
                for name in ('stdout','stderr'): streams.append((base/name).open('xb'))
            child=subprocess.Popen(argv, **({} if terminal else {'stdout':streams[0],'stderr':streams[1]}))
            created=self.write(base/'created.json',{'schema':'production_native_created_v1','intent':intent,
                'pid':child.pid,'argv':argv,'wall_time_ns':time.time_ns(),'ownership':'direct_popen_child'})
            rc=child.wait(timeout=timeout)
            observed_rc=rc;waited=True
            for stream in streams: stream.flush()
            values=[]
            for name in ('stdout','stderr'):
                if terminal: values.append(None); continue
                path=base/name; m.require(path.stat().st_size<=MAX_OUTPUT,'native output exceeds bound')
                values.append(path.read_bytes().decode('utf-8',errors='strict'))
            result=self.write(base/'exit.json',{'schema':'production_native_exit_v1','intent':intent,'created':created,
                'pid':child.pid,'argv':argv,'returncode':rc,'naturally_reaped':True,'signals_sent':0,
                'wall_time_ns':time.time_ns(),'stdout':values[0],'stderr':values[1]})
            self.failed=False
            return SimpleNamespace(returncode=rc,stdout=values[0],stderr=values[1],native_command=result)
        except BaseException as error:
            self.failed=True
            known=observed_rc if waited else None
            if child is not None and not waited:
                try: known=child.poll()
                except BaseException: pass
            # Failure to publish this record still leaves the one-use durable
            # intent. No caller may infer no child merely from missing created.
            try:
                self.write(base/'unresolved.json',{'schema':'production_native_unresolved_v1','intent':intent,
                    'created':created,'pid':None if child is None else child.pid,'argv':argv,
                    'error_type':type(error).__name__,'error':str(error)[:1500],
                    'naturally_reaped':known is not None,'known_returncode':known,
                    'may_still_be_alive':child is not None and known is None,
                    'publication_uncertain':child is not None and created is None,
                    'signals_sent':0,'retry_permitted':False,'wall_time_ns':time.time_ns()})
            except BaseException: pass
            raise
        finally:
            # Closing our output handles neither waits for nor signals a child.
            for stream in streams: stream.close()


def retained_commands(root, plan, evidence):
    """Bind command histories and report uncertainty; never reconcile an effect."""
    parent=Path(root)/'native-commands'
    if not parent.exists(): return []
    unresolved=[]
    m.require(parent==parent.resolve(strict=True),'aliased native command history')
    for attempt in sorted(parent.iterdir()):
        m.require(attempt.is_dir() and attempt==attempt.resolve(strict=True),'foreign native attempt')
        owner=evidence.read(attempt/'owner.json',record=True)
        m.require(owner.get('schema')=='production_native_effect_owner_v1'
                  and owner['root']==str(attempt) and owner['transaction_id']==plan['transaction_id']
                  and owner['plan_sha256']==m.digest(plan),'native attempt belongs elsewhere')
        session=owner['executor_session']
        m.require(session['plan_sha256']==m.digest(plan) and session['root']==str(root)
                  and session['executor_identity']['pid']==owner['pid'],'native executor/session differs')
        saved_session=evidence.read(Path(root)/'execution-sessions'/(session['session_id']+'-begin.json'),record=True)
        m.require(saved_session==session,'native session missing')
        for index,base in enumerate(sorted(p for p in attempt.iterdir() if p.name!='owner.json'),1):
            m.require(base.name==f'command-{index:05d}' and base.is_dir() and not base.is_symlink(),'native command ordering differs')
            paths=set(base.iterdir()); intent_path=base/'intent.json'
            if intent_path not in paths:
                unresolved.append(str(base)+': missing command intent; no publication success inferred');continue
            intent=evidence.read(intent_path,record=True)
            m.require(intent.get('owner')==owner and intent.get('command_id')==index,'native intent owner/order differs')
            for path in paths:
                m.require(path.name in ('intent.json','created.json','exit.json','unresolved.json','stdout','stderr'), 'unexpected native record')
                evidence.read(path,record=path.suffix=='.json')
            if base/'unresolved.json' in paths or not {base/'created.json',base/'exit.json'}<=paths:
                unresolved.append(str(base)+': command outcome/publication unresolved; no repeat authorized');continue
            created=evidence.read(base/'created.json',record=True);end=evidence.read(base/'exit.json',record=True)
            iref={'path':str(intent_path),'sha256':evidence.read(intent_path)}
            cref={'path':str(base/'created.json'),'sha256':evidence.read(base/'created.json')}
            m.require(created['intent']==iref and end['intent']==iref and end['created']==cref
                      and created['argv']==intent['argv']==end['argv'] and created['pid']==end['pid']
                      and type(created['pid']) is int and created['pid']>1 and end['naturally_reaped'] is True
                      and end['signals_sent']==0 and type(end['returncode']) is int,'native natural reap proof differs')
            m.require(intent['wall_time_ns']<=created['wall_time_ns']<=end['wall_time_ns'],'native command chronology differs')
    return unresolved


def host_links(root, plan, host_events, evidence):
    """Host completion must name the exact naturally reaped direct control child."""
    kinds={'debug_result':'debug','bootstrap_command_returned':'bootstrap','held_job_absent':'bootout'}
    root=Path(root)
    for event in host_events:
        kind=kinds.get(event['operation'])
        if kind is None: continue
        link=event.get('native_command')
        m.require(isinstance(link,dict) and set(link)=={'path','sha256'},'Host native command reference missing')
        path=Path(link['path'])
        m.require(path.name=='exit.json' and path.parent.parent.parent==root/'native-commands'
                  and evidence.read(path)==link['sha256'],'Host native completion outside fixed attempt')
        end=evidence.read(path,record=True);intent_path=Path(end['intent']['path'])
        m.require(intent_path==path.with_name('intent.json') and evidence.read(intent_path)==end['intent']['sha256'],'Host native intent differs')
        intent=evidence.read(intent_path,record=True)
        role=event['role'];target=f"gui/{plan['host_binding']['uid']}/{m.LABELS[role]}"
        argv=intent['argv']
        if kind=='debug':m.require(argv[:5]==['/usr/bin/sudo','--','/bin/launchctl','debug',target],'Host native debug target differs')
        elif kind=='bootout':m.require(argv==['/bin/launchctl','bootout',target],'Host native bootout target differs')
        else:m.require(argv==['/bin/launchctl','bootstrap',f"gui/{plan['host_binding']['uid']}",
                            str(Path('/Users/v/Library/LaunchAgents')/(m.LABELS[role]+'.plist'))],'Host native bootstrap target differs')
        m.require(intent['kind']==kind and end['naturally_reaped'] is True and end['signals_sent']==0
                  and (kind=='bootout' and end['returncode']==0 or event.get('returncode')==end['returncode']),
                  'Host native return/reap differs')
