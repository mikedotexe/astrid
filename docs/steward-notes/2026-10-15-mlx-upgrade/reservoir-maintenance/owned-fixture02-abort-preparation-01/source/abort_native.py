"""Native boundary for the one fixed abort. Importing this module does nothing."""
from __future__ import annotations
from contextlib import contextmanager
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import signal
import stat
import subprocess
import sys
import time

import abort_evidence as e


class NativeHost:
    scope='failed_owned_fixture_abort_only'

    def __init__(self,t,snapshot,release):
        self.t=t;self.snapshot=snapshot;self.release=release;self.root=Path(t['run_root'])
        self.abort=Path(t['abort_root']);self.queries=[];self.query_bytes=0;self.active=False;self.lock_fd=None
        self.created_children=[];self.signal_used=False;self.debug_used=False;self.removed=set();self.owner=None
        self.debug_confirmation=None;self.saved=False;self.provider_released=False
        self.pending_holders={}

    def query(self,argv):
        e.require(argv[0] in ('/bin/ps','/usr/sbin/lsof','/usr/sbin/sysctl','/usr/bin/sw_vers')
                  or argv[:2]==['/bin/launchctl','print'],'action command cannot use read-only query path')
        child=None
        try:
            child=subprocess.Popen(argv,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            self.created_children.append(child)
            stdout,stderr=child.communicate(timeout=5)
            row={'argv':argv,'pid':child.pid,'returncode':child.returncode,'stdout':stdout,'stderr':stderr,
                 'naturally_reaped':True,'observed_wall_time_ns':time.time_ns()}
        except BaseException as error:
            row={'argv':argv,'pid':None if child is None else child.pid,'unresolved':True,'signals_sent':0,
                 'may_still_be_alive':child is not None and child.returncode is None,
                 'naturally_reaped':child is not None and child.returncode is not None,
                 'known_returncode':None if child is None else child.returncode,
                 'error_type':type(error).__name__,'observed_wall_time_ns':time.time_ns()}
            self.queries.append(row)
            raise RuntimeError('read-only query unresolved; no kill or retry: '+json.dumps(row)) from error
        self.query_bytes+=len(json.dumps(row));e.require(len(self.queries)<6000 and self.query_bytes<8*1024*1024,'query evidence bound exceeded')
        self.queries.append(row);return row

    def host_binding(self):
        boot=self.query(['/usr/sbin/sysctl','-n','kern.bootsessionuuid']);version=self.query(['/usr/bin/sw_vers','-productVersion'])
        e.require(boot['returncode']==version['returncode']==0,'host identity query failed')
        return {'uid':os.getuid(),'boot_uuid':boot['stdout'].strip(),'hostname':os.uname().nodename,'os_version':version['stdout'].strip()}

    def start(self,pid):
        row=self.query(['/bin/ps','-p',str(pid),'-o','lstart='])
        if row['returncode']:
            e.require(row['returncode']==1 and not row['stdout'].strip() and not row['stderr'].strip(),'ambiguous process absence')
            return None
        e.require(row['stdout'].strip() and not row['stderr'].strip(),'missing process start');return row['stdout'].strip()

    def process(self,pid):
        if pid is None:return None
        e.require(type(pid) is int and pid>1,'invalid PID');before=self.start(pid)
        if before is None:return None
        row=self.query(['/bin/ps','-ww','-p',str(pid),'-o','command='])
        if row['returncode']:
            e.require(row['returncode']==1 and not row['stdout'].strip() and not row['stderr'].strip()
                      and self.start(pid) is None,'unproved command disappearance');return None
        after=self.start(pid)
        if after is None:return None
        e.require(before==after and row['stdout'].strip() and not row['stderr'].strip(),'unstable process identity')
        return {'pid':pid,'started_at':before,'argv':shlex.split(row['stdout'].strip())}

    def executor(self):
        value=self.process(os.getpid());e.require(value is not None,'abort executor identity unavailable');return value

    def job(self,role):
        e.require(role in ('provider','reservoir','pin'),'unknown role')
        row=self.query(['/bin/launchctl','print',self.t['targets'][role]])
        if row['returncode']:
            e.require('Could not find service' in row['stdout']+row['stderr'] or 'Could not find specified service' in row['stdout']+row['stderr'], 'job absence unproved')
            return False,None
        found=re.search(r'^\s*pid = (\d+)\s*$',row['stdout'],re.M)
        return True,int(found[1]) if found else None

    def listeners(self,pid):
        row=self.query(['/usr/sbin/lsof','-nP','-a','-p',str(pid),'-iTCP','-sTCP:LISTEN','-Fpn'])
        e.require(row['returncode'] in (0,1) and not row['stderr'].strip(),'listener query failed')
        ports=[]
        for line in row['stdout'].splitlines():
            if line.startswith('p'):e.require(line=='p'+str(pid),'foreign listener owner')
            if line.startswith('n'):
                match=re.fullmatch(r'n127\.0\.0\.1:(\d+)',line);e.require(match is not None,'nonprivate listener')
                ports.append(int(match[1]))
        e.require(row['returncode']==0 or not row['stdout'].strip(),'ambiguous listener absence');return ports

    def port_pids(self,role):
        row=self.query(['/usr/sbin/lsof','-nP',f'-iTCP:{self.t["ports"][role]}','-sTCP:LISTEN','-Fp'])
        e.require(row['returncode'] in (0,1) and not row['stderr'].strip()
                  and (row['returncode']==0 or not row['stdout'].strip()),'private port query failed')
        return sorted({int(line[1:]) for line in row['stdout'].splitlines() if line.startswith('p')})

    def coordination(self):
        value=self.t['coordination'];lease=Path(value['lease_path'])
        e.require(not os.path.lexists(lease),'steward lease exists')
        state=e.checked(value['control'])
        e.require(state.get('schema')=='steward_control_state_v1' and state.get('schema_version')==1
            and state.get('paused') is True and state.get('pause_generation')==528 and state.get('actor')=='codex-mlx-upgrade',
            'steward owner changed')
        e.require(not os.path.lexists(lease),'steward lease appeared');return value

    def protected(self):
        for pid,expected in self.t['protected_exact'].items():
            row=self.query(['/bin/ps','-ww','-p',pid,'-o','pid=,ppid=,lstart=,command='])
            e.require(row['returncode']==0 and row['stdout'].strip()==expected.strip() and not row['stderr'].strip(), 'protected process changed')

    def old_executors_absent(self):
        identities=[self.t['parent_identity'],self.t['workers']['provider']]
        for path in sorted((self.root/'executors').glob('*/self-start.json')):identities.append(e.read(path)['identity'])
        e.require(len(identities)==9,'retained executor set differs')
        for ident in identities:e.require(self.process(ident['pid']) is None,'old parent/executor/provider survives or PID reused')

    def control(self,role):
        e.require(role in ('provider','reservoir'),'unknown control role')
        return self.root/'hold-provider' if role=='provider' else self.abort/'hold-reservoir'

    def guard(self,role):return self.t[role+'_guard']

    def hold_argv(self,role):
        return [self.t['workers']['provider']['argv'][0],str(self.root/'model_environment.py'),'launch',
                '--control-root',str(self.control(role)),'--hold-only',self.guard(role)['transaction_id']]

    def inspect(self,role):
        loaded,pid=self.job(role);process=self.process(pid)
        if pid is not None and process is None:
            # Corroborate the old-PID exit; no foreign replacement is accepted.
            loaded,new_pid=self.job(role)
            if new_pid==pid and loaded and role=='reservoir' and self.signal_used and pid==self.t['workers']['reservoir']['pid']:
                e.require(self.process(pid) is None and not self.port_pids(role)
                          and e.read(self.control(role)/'hold.json')==self.guard(role),'unproved old-worker publication gap')
                return {'loaded':True,'identity':None,'held':False,'port_pids':[],
                        'publication_pending':'signalled original worker absent; launchd still publishes its PID'}
            e.require(new_pid!=pid,'stale disappeared PID remains published');pid=new_pid;process=self.process(pid)
        ports=self.port_pids(role)
        view={'loaded':loaded,'identity':process,'held':False,'port_pids':ports}
        if process is None:return view
        own_ports=self.listeners(process['pid'])
        if process==self.t['workers'][role]:
            e.require(role=='reservoir' and own_ports in ([self.t['ports'][role]],[])
                      and ports in ([process['pid']],[]),'unexpected original worker/listener')
            return view
        expected=self.hold_argv(role);actual=process['argv']
        family=set(self.t['python_files'])|{self.t['python']}
        e.require(actual[1:]==expected[1:] and actual[0] in family and not own_ports and not ports,
                  'foreign holder/listener')
        if role=='provider':e.require(process==self.t['provider_holder'],'provider holder changed')
        prior=self.pending_holders.setdefault(role,{'pid':process['pid'],'started_at':process['started_at']})
        e.require(prior=={'pid':process['pid'],'started_at':process['started_at']},'holder publication identity changed')
        guard=e.read(self.control(role)/'hold.json');e.require(guard==self.guard(role),'holder guard differs')
        view['holder_pending']=True
        path=self.control(role)/'held'/(str(process['pid'])+'.json')
        if not path.exists():return view  # Exact launcher may publish receipt next.
        held=e.read(path)
        e.require(held=={'schema':'model_environment_held_v1','pid':process['pid'],'started_at':process['started_at'],
            'hold':guard,'hold_only':True,'launcher_sha256':e.sha(self.root/'model_environment.py'),
            'model_imported':False,'listener_opened':False},'holder receipt differs')
        if actual!=expected:return view  # Exact pinned launcher family is pending, never accepted as held.
        view['held']=True;view['holder_pending']=False;view['receipt']=e.reference(path);return view

    @contextmanager
    def lock(self):
        path=self.root/'execution.lock';fd=os.open(path,os.O_RDWR|os.O_NOFOLLOW)
        try:
            info=os.fstat(fd)
            e.require(stat.S_ISREG(info.st_mode) and info.st_uid==os.getuid() and info.st_nlink==1
                and info.st_size==0 and not stat.S_IMODE(info.st_mode)&0o077
                and {'device':info.st_dev,'inode':info.st_ino}==self.t['execution_lock_identity'],'foreign execution lock')
            fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)
            e.require(e.stamp(info)==e.stamp(path.lstat()),'execution lock replaced')
            self.lock_fd=fd;yield {'device':info.st_dev,'inode':info.st_ino}
        finally:self.active=False;self.lock_fd=None;os.close(fd)

    def allow(self):
        e.require(self.active and self.lock_fd is not None and self.owner is not None,'abort mutation authority absent')
        e.require(e.read(self.abort/'owner.json')==self.owner,'abort owner changed')
        e.checked(self.owner['authorization'])
        info=os.fstat(self.lock_fd);e.require(e.stamp(info)==e.stamp((self.root/'execution.lock').lstat()),'execution lock changed')
        e.require(self.executor()==self.owner['executor_identity'],'abort executor changed')
        self.coordination();e.require(self.host_binding()==self.t['host_binding'],'host binding changed')
        e.require(e.source_artifacts()==self.owner['artifacts'],'abort helper bytes changed')
        e.verify_sources(self.t,self.release);self.protected();self.old_executors_absent()
        e.verify_original(self.t,self.snapshot,saved=self.saved,provider_released=self.provider_released)
        e.checkin_proof(self.t,saved=self.saved)

    def activate(self,owner):
        e.require(self.lock_fd is not None and os.getuid()==501 and os.geteuid()==501 and sys.stdin.isatty(),
                  'ordinary-user Terminal under shared lock required')
        self.owner=owner;self.active=True;self.allow()

    def effect(self,name,argv,record,timeout,precondition):
        """Owned action child: never kill/terminate on timeout or interruption."""
        self.allow();record('command_launch',name=name,argv=argv)
        child=None;code=None;reaped=False
        try:
            precondition()  # Recheck exact worker/holder after the potentially lengthy full hash pass.
            child=subprocess.Popen(argv)  # Inherit Terminal; never capture credentials.
            self.created_children.append(child)
            record('command_created',name=name,pid=child.pid,argv=argv,identity=self.process(child.pid))
            code=child.wait(timeout=timeout);reaped=True
            record('command_exit',name=name,pid=child.pid,argv=argv,returncode=code,naturally_reaped=True)
        except BaseException as error:
            record('command_unresolved',name=name,pid=None if child is None else child.pid,argv=argv,
                   error_type=type(error).__name__,error=str(error)[:500],may_still_be_alive=child is not None and not reaped,
                   naturally_reaped=reaped,known_returncode=code if reaped else None,signals_sent=0,retry_permitted=False)
            raise
        e.require(code==0,'action child nonzero; no retry')
        return {'pid':child.pid,'returncode':code,'naturally_reaped':True}

    def create_guard(self,record):
        self.allow();path=self.control('reservoir');e.require(not path.exists(),'abort reservoir guard already exists')
        path.mkdir(mode=0o700);write_new(path/'hold.json',self.guard('reservoir'));return e.reference(path/'hold.json')

    def debug(self,record):
        self.allow();e.require(not self.debug_used,'debug already attempted');self.debug_used=True
        self.ready_reservoir()
        e.require(e.read(self.control('reservoir')/'hold.json')==self.guard('reservoir'),'abort guard changed')
        argv=['/usr/bin/sudo','--','/bin/launchctl','debug',self.t['targets']['reservoir'],'--program',self.t['python'],
              '--',*self.hold_argv('reservoir')[1:]]
        def ready():
            self.ready_provider_holder()
            self.ready_reservoir()
            e.require(e.read(self.control('reservoir')/'hold.json')==self.guard('reservoir'),'abort guard changed')
        result=self.effect('debug_reservoir',argv,record,180,ready)
        self.debug_confirmation={'argv':argv,'result':result};return result

    def ready_reservoir(self):
        view=self.inspect('reservoir')
        e.require(view['loaded'] and view['identity']==self.t['workers']['reservoir'] and not view['held']
                  and view['port_pids']==[self.t['workers']['reservoir']['pid']],'original reservoir sentinel changed')

    def ready_provider_holder(self):
        view=self.inspect('provider')
        e.require(view['loaded'] and view['held'] and view['identity']==self.t['provider_holder'] and not view['port_pids'],
                  'original provider holder changed')

    def signal_reservoir(self,record):
        self.allow();e.require(not self.signal_used,'signal already attempted');self.signal_used=True
        self.ready_reservoir()
        rows=[e.read(path) for path in sorted(self.abort.glob('event-*.json'))]
        returns=[row for row in rows if row['type']=='result' and row.get('action')=='debug_reservoir']
        e.require(self.debug_confirmation is not None and len(returns)==1
                  and returns[0]['result']==self.debug_confirmation['result']
                  and returns[0]['result']['returncode']==0 and returns[0]['result']['naturally_reaped'] is True,
                  'exact durable successful debug required before signal')
        self.ready_provider_holder();self.ready_reservoir()
        e.require(e.read(self.control('reservoir')/'hold.json')==self.guard('reservoir'),'abort guard changed before signal')
        os.kill(self.t['workers']['reservoir']['pid'],signal.SIGTERM)
        return {'identity':self.t['workers']['reservoir'],'signal':'SIGTERM','force_used':False}

    def remove_held(self,role,expected,record):
        self.allow();e.require(role in ('provider','reservoir') and role not in self.removed,'holder removal already attempted')
        view=self.inspect(role);e.require(view['loaded'] and view['held'] and view['identity']==expected and not view['port_pids'],
                                       'only exact no-listener holder may be removed')
        self.removed.add(role)
        def held():
            current=self.inspect(role)
            e.require(current['loaded'] and current['held'] and current['identity']==expected and not current['port_pids'],
                      'exact holder changed before bootout')
        return self.effect('bootout_'+role,['/bin/launchctl','bootout',self.t['targets'][role]],record,20,held)

    def release_guard(self,role,expected,record):
        self.allow();loaded,pid=self.job(role)
        e.require(not loaded and pid is None and self.process(expected['pid']) is None and not self.port_pids(role),
                  'job/PID/listener absence required before guard release')
        path=self.control(role)/'hold.json';e.require(e.read(path)==self.guard(role),'guard ownership changed')
        path.unlink();sync_directory(path.parent)
        if role=='provider':self.provider_released=True
        return {'released':True,'guard':self.guard(role)}

    def verify(self,*,saved=False,provider_released=False):
        self.coordination();e.require(self.host_binding()==self.t['host_binding'],'host changed')
        self.protected();self.old_executors_absent()
        e.require(self.job('pin')==(False,None),'pin job unexpectedly exists')
        original=e.verify_original(self.t,self.snapshot,saved=saved,provider_released=provider_released)
        proof=e.checkin_proof(self.t,saved=saved)
        self.saved=saved;self.provider_released=provider_released
        return {'original':original,'checkin':proof,'host_binding':self.t['host_binding']}

    def verify_signal_boundary(self,intent):
        rows=e.event_rows(self.root/'reservoir.events.jsonl');info=(self.root/'state.json').stat()
        e.require(intent['action']=='signal_reservoir' and intent['identity']==self.t['workers']['reservoir']
                  and rows[3]['monotonic']>=intent['monotonic'] and info.st_mtime_ns>=intent['wall_time_ns'],
                  'final save does not follow the exact abort signal intent')

    def capture_saved(self):
        """Embed exact final bytes in the fsynced save-proof record before cleanup."""
        captured={}
        for name in ('state.json','reservoir.events.jsonl'):
            path=self.root/name;data=e.raw(path);text=data.decode('utf-8');ref=e.reference(path)
            e.require(text.encode('utf-8')==data and hashlib.sha256(data).hexdigest()==ref['sha256'],
                      'final save changed while capturing immutable copy')
            captured[name]={'source':ref,'bytes':len(data),'utf8':text,'sha256':ref['sha256']}
        return captured

    def final_evidence(self):
        sources=e.verify_sources(self.t,self.release)
        e.require(all(not os.path.lexists(self.control(role)/'hold.json') for role in ('provider','reservoir')), 'abort guards remain')
        return {'sources':sources,'state':e.reference(self.root/'state.json'),
                'reservoir_log':e.reference(self.root/'reservoir.events.jsonl'),
                'old_workers_absent':True,'holders_absent':True,'guards_absent':True,
                'observation_kind':'OS absence, not waitpid of launchd workers'}

    monotonic=staticmethod(time.monotonic)
    sleep=staticmethod(time.sleep)


def sync_directory(path):
    fd=os.open(path,os.O_RDONLY)
    try:os.fsync(fd)
    finally:os.close(fd)


def write_new(path,value):
    path=Path(path)
    e.require(path.is_relative_to(e.ABORT) and path.parent.resolve(strict=True)==path.parent,'write outside abort evidence root')
    data=(json.dumps(value,sort_keys=True,indent=2,allow_nan=False)+'\n').encode()
    e.require(len(data)<=e.LIMIT,'abort record too large')
    fd=os.open(path,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
    with os.fdopen(fd,'wb') as stream:stream.write(data);stream.flush();os.fsync(stream.fileno())
    sync_directory(path.parent)
