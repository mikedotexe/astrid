"""One-use owned native commands with durable evidence and no forced cleanup.

No work at import. Only the admitted fixed fixture parent/executor configures a
caller. The caller retains service authority; this helper never retries or signals.
"""
from __future__ import annotations
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import subprocess
import sys
import time
from types import SimpleNamespace

import owned_fixture_records as r

HERE=Path(__file__).resolve().parent
MAX_ROWS=12000
MAX_ROW=262144
MAX_BYTES=64*1024*1024
_context=None


def native_root(release):return HERE/'runs'/('owned-native-'+release['fixture']['token'])


def native_reference(path):return {'path':str(path),'sha256':hashlib.sha256(r.raw(path,limit=MAX_BYTES)).hexdigest()}


def _sync(path):
    fd=os.open(path,os.O_RDONLY)
    try:os.fsync(fd)
    finally:os.close(fd)


def _stamp(info):return (info.st_dev,info.st_ino,info.st_size,info.st_mtime_ns,info.st_ctime_ns,info.st_mode,info.st_uid,info.st_nlink)


def _enabled():
    from owned_fixture_coordinator import FIXTURE_EXECUTION_ENABLED
    r.require(FIXTURE_EXECUTION_ENABLED is True,'owned native commands disabled')


def configure(release,release_ref,caller):
    """Reserve caller once, before even its first read-only native query."""
    global _context
    _enabled();r.require(_context is None,'native caller already configured')
    r.require(r.reference(release_ref['path'])==release_ref,'native release changed')
    fixture=Path(release['fixture']['root']);root=native_root(release);pid=os.getpid()
    r.require(os.getuid()==release['host_binding']['uid']>0 and os.geteuid()!=0,'native caller must be ordinary owning user')
    r.require(str(Path(sys.executable).resolve(strict=True)) in release['python_files'],'native caller Python differs')
    if caller=='parent':
        argv=[release['python'],'-B',str(HERE/'owned_fixture_run.py'),'--execute','--release',release_ref['path']]
        r.require(sys.argv==argv[2:] and not root.exists() and not root.is_symlink(),'parent native attempt retained or argv differs')
        r.directory(root.parent);root.mkdir(mode=0o700);_sync(root.parent)
        owner={'schema':'owned_fixture_native_root_v1','root':str(root),'fixture_root':str(fixture),
               'token':release['fixture']['token'],'release':release_ref,'parent_pid':pid,'parent_expected_argv':argv,
               'python':release['python'],'python_files':release['python_files'],'created_wall_time_ns':time.time_ns()}
        r.write(root/'owner.json',owner)
    else:
        r.require(caller in [row['id'] for row in release['executor_plan']],'unsealed native caller')
        request=fixture/'executors'/caller/'request.json'
        argv=[release['python'],'-B',str(HERE/'owned_fixture_coordinator.py'),'--executor-request',str(request)]
        r.require(sys.argv==argv[2:] and r.read(request)['release']==release_ref,'executor native argv/release differs')
        owner=r.read(root/'owner.json')
        r.require(owner['schema']=='owned_fixture_native_root_v1' and owner['root']==str(root)
                  and owner['fixture_root']==str(fixture) and owner['token']==release['fixture']['token']
                  and owner['release']==release_ref and owner['python']==release['python']
                  and owner['python_files']==release['python_files'],'native root owner differs')
    base=r.directory(root)/caller;base.mkdir(mode=0o700);_sync(root)
    caller_owner={'schema':'owned_fixture_native_caller_v1','root_owner':r.reference(root/'owner.json'),
                  'caller':caller,'pid':pid,'expected_argv':argv,'release':release_ref,'created_wall_time_ns':time.time_ns()}
    owner_ref=r.write(base/'owner.json',caller_owner)
    _context=Commands(release,release_ref,base,caller_owner,owner_ref)
    return owner_ref


class Commands:
    def __init__(self,release,release_ref,base,owner,owner_ref):
        self.release=release;self.release_ref=release_ref;self.base=base;self.owner=owner;self.owner_ref=owner_ref
        self.path=base/'commands.jsonl';self.previous=r.digest(owner);self.rows=0;self.command_id=0;self.failed=False
        self.children=[];self.fd=os.open(self.path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
        os.fsync(self.fd);_sync(base);self.stamp=_stamp(os.fstat(self.fd))

    def append(self,event_type,command_id,**details):
        r.require(self.rows<MAX_ROWS and _stamp(self.path.lstat())==self.stamp==_stamp(os.fstat(self.fd)),
                  'native command journal changed or exhausted')
        row={'schema':'owned_fixture_native_event_v1','sequence':self.rows+1,'previous_sha256':self.previous,
             'type':event_type,'command_id':command_id,'caller_owner_sha256':r.digest(self.owner),
             'wall_time_ns':time.time_ns(),'monotonic':time.monotonic(),**details}
        data=(json.dumps(row,sort_keys=True,separators=(',',':'),allow_nan=False)+'\n').encode()
        r.require(len(data)<=MAX_ROW and self.stamp[2]+len(data)<=MAX_BYTES,'native journal byte bound exceeded')
        offset=0
        while offset<len(data):
            count=os.write(self.fd,data[offset:]);r.require(count>0,'native journal write made no progress');offset+=count
        os.fsync(self.fd);self.stamp=_stamp(os.fstat(self.fd));self.rows+=1;self.previous=r.digest(row)
        return row

    def verify(self,argv,terminal,timeout):
        _enabled();r.require(not self.failed and os.getpid()==self.owner['pid'],'native caller failed or changed')
        r.require(r.read(self.base/'owner.json')==self.owner and r.reference(self.release_ref['path'])==self.release_ref,
                  'native ownership/release changed')
        r.require(r.reference(__file__)['sha256']==self.release['artifacts'][str(HERE/'owned_fixture_commands.py')],
                  'native helper changed')
        r.require(isinstance(argv,list) and argv and all(isinstance(x,str) and x for x in argv)
                  and type(timeout) is int and timeout in (20,180) and type(terminal) is bool,'native command shape differs')
        token=self.release['fixture']['token'];uid=self.release['host_binding']['uid'];root=Path(self.release['fixture']['root'])
        targets={f'gui/{uid}/com.codex.reservoir-maintenance-fixture.{token}.{role}' for role in ('provider','reservoir','pin')}
        bare=argv[2:] if argv[:2]==['/usr/bin/sudo','--'] else argv
        if bare[:2]==['/bin/launchctl','print']:
            r.require(len(bare)==3 and bare[2] in targets and bare==argv and not terminal and timeout==20,
                      'foreign print command');return 'query'
        if bare and bare[0]=='/bin/launchctl':
            r.require(len(bare)>1,'incomplete launchctl command')
            operation=bare[1]
            if operation=='bootstrap':r.require(len(bare)==4 and bare[2]==f'gui/{uid}' and Path(bare[3]) in
                    (root/'provider.plist',root/'reservoir.plist',root/'pin/active.plist'),'foreign bootstrap')
            elif operation=='bootout':r.require(len(bare)==3 and bare[2] in targets,'foreign bootout')
            elif operation=='debug':
                r.require(self.release.get('sudo_debug') is True and terminal
                          and argv[:2]==['/usr/bin/sudo','--'],'debug privilege form differs from reviewed release')
                roles=('provider','reservoir')
                expected=[['/bin/launchctl','debug',f'gui/{uid}/com.codex.reservoir-maintenance-fixture.{token}.{role}',
                    '--program',self.release['python'],'--',str(root/'model_environment.py'),'launch',
                    '--control-root',str(root/('hold-'+role)),'--hold-only',guard]
                    for role in roles for guard in (token,token+'-cleanup')]
                r.require(bare in expected,'foreign debug')
            else:raise ValueError('unapproved launchctl operation')
            r.require(terminal==(argv[:2]==['/usr/bin/sudo','--']) and (not terminal or operation=='debug')
                      and timeout==(180 if terminal else 20),'native action timeout/Terminal differs');return 'effect'
        pid=lambda value:isinstance(value,str) and re.fullmatch(r'[1-9][0-9]*',value) is not None and int(value)>1
        allowed=argv in (['/usr/sbin/sysctl','-n','kern.bootsessionuuid'],['/usr/bin/sw_vers','-productVersion'])
        allowed=allowed or (len(argv)==5 and argv[:2]==['/bin/ps','-p'] and pid(argv[2]) and argv[3:]==['-o','lstart='])
        allowed=allowed or (len(argv)==6 and argv[:3]==['/bin/ps','-ww','-p'] and pid(argv[3]) and argv[4:]==['-o','command='])
        allowed=allowed or (len(argv)==8 and argv[:4]==['/usr/sbin/lsof','-nP','-a','-p'] and pid(argv[4])
                            and argv[5:]==['-iTCP','-sTCP:LISTEN','-Fpn'])
        if len(argv)==5 and argv[:2]==['/usr/sbin/lsof','-nP'] and argv[3:]==['-sTCP:LISTEN','-Fp']:
            match=re.fullmatch(r'-iTCP:([1-9][0-9]*)',argv[2])
            allowed=allowed or (match is not None and 1024<int(match[1])<65536
                                and int(match[1]) not in (7878,7879,7880,7881,8090,8092,17881,11434))
        r.require(not terminal and timeout==20 and allowed,'unapproved native query')
        return 'query'

    def run(self,argv,*,terminal=False,timeout=20):
        kind=self.verify(argv,terminal,timeout);self.command_id+=1;number=self.command_id
        try:intent=self.append('intent',number,argv=argv,terminal=terminal,timeout_s=timeout,kind=kind)
        except BaseException:self.failed=True;raise
        created=None;child=None;code=None;reaped=False;stdout=stderr=None if terminal else ''
        try:
            child=subprocess.Popen(argv,**({} if terminal else {'stdout':subprocess.PIPE,'stderr':subprocess.PIPE,'text':True}))
            self.children.append(child)
            created=self.append('created',number,pid=child.pid,argv=argv,intent_sha256=r.digest(intent))
            if terminal:code=child.wait(timeout=timeout)
            else:stdout,stderr=child.communicate(timeout=timeout);code=child.returncode
            reaped=True
            r.require(type(code) is int and (terminal or len(stdout.encode())+len(stderr.encode())<128*1024),
                      'native command output/return exceeds bound')
            ended=self.append('exit',number,pid=child.pid,argv=argv,intent_sha256=r.digest(intent),
                    created_sha256=r.digest(created),returncode=code,naturally_reaped=True,stdout=stdout,stderr=stderr,signals_sent=0)
        except BaseException as error:
            self.failed=True
            # If publication itself failed, original intent plus durable partial
            # bytes remain. Best-effort diagnostic never replaces a failed row.
            known=code if reaped else (None if child is None else child.returncode)
            diagnostic={'pid':None if child is None else child.pid,'argv':argv,'intent_sha256':r.digest(intent),
              'created_sha256':None if created is None else r.digest(created),'error_type':type(error).__name__,
              'error':str(error)[:1000],'naturally_reaped':reaped or known is not None,'known_returncode':known,
              'may_still_be_alive':child is not None and not reaped and known is None,'signals_sent':0,'retry_permitted':False}
            try:self.append('unresolved',number,**diagnostic)
            except BaseException:
                try:r.write(self.base/f'publication-failure-{number:05d}.json',diagnostic)
                except BaseException:pass
            raise
        link={'path':str(self.path),'command_id':number,'intent_sha256':r.digest(intent),
              'created_sha256':r.digest(created),'exit_sha256':r.digest(ended)}
        return SimpleNamespace(returncode=code,stdout=stdout,stderr=stderr,native_command=link)


def run(argv,*,terminal=False,timeout=20):
    r.require(_context is not None,'native caller has no durable ownership')
    return _context.run(list(argv),terminal=terminal,timeout=timeout)


def capture(release):
    """Read completed native trees only; no new commands or state changes."""
    root=r.directory(native_root(release));owner=r.reference(root/'owner.json');callers=[];artifacts={};total=0
    expected={'parent',*(row['id'] for row in release['executor_plan'])}
    r.require({path.name for path in root.iterdir()}==expected|{'owner.json'},'native caller set differs')
    for name in sorted(expected):
        base=r.directory(root/name)
        r.require({path.name for path in base.iterdir()}=={'owner.json','commands.jsonl'},'native unresolved/extra artifact')
        caller=r.read(base/'owner.json');r.require(caller['caller']==name and caller['root_owner']==owner,'native caller owner differs')
        data=r.raw(base/'commands.jsonl',limit=MAX_BYTES);r.require(not data or data.endswith(b'\n'),'partial native command journal')
        rows=[json.loads(line) for line in data.splitlines()]
        r.require(len(rows)%3==0 and all(row['type']==('intent','created','exit')[i%3]
                  and (row['type']!='exit' or row['naturally_reaped'] is True) for i,row in enumerate(rows)),
                  'native command incomplete or unresolved')
        callers.append({'caller':name,'owner':r.reference(base/'owner.json'),'log':native_reference(base/'commands.jsonl')})
    for path in [root/'owner.json',*(root/x/y for x in sorted(expected) for y in ('owner.json','commands.jsonl'))]:
        total+=path.stat().st_size;r.require(total<=MAX_BYTES,'native run exceeds byte bound');artifacts[str(path)]=native_reference(path)['sha256']
    return {'root':str(root),'owner':owner,'callers':callers,'artifacts':artifacts}
